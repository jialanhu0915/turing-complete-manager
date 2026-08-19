//! `tc-mod-inject.exe` — DLL injection tool.
//!
//! Injects a given DLL into a target Windows process using the classic
//! `CreateRemoteThread` + `LoadLibraryW` technique.
//!
//! # Usage
//!
//! ```text
//! tc-mod-inject.exe <pid> <path\to\td_mod_hook.dll>
//! ```
//!
//! # How it works
//!
//! 1. `OpenProcess(pid, PROCESS_ALL_ACCESS)` — get a handle to the target
//! 2. `VirtualAllocEx` + `WriteProcessMemory` — copy the DLL path string into
//!    the target's address space
//! 3. `GetModuleHandleW("kernel32.dll")` + `GetProcAddress("LoadLibraryW")` —
//!    find `LoadLibraryW` in our own process (kernel32 is mapped at the same
//!    address in every Windows process thanks to the kernel's per-process
//!    handle table)
//! 4. `CreateRemoteThread(load_library_w, path_ptr)` — start a thread in the
//!    target that calls `LoadLibraryW(path)`. Windows will load our DLL into the
//!    target and call its `DllMain(DLL_PROCESS_ATTACH)`.
//! 5. Wait for the thread, then clean up the remote buffer + handles.
//!
//! # Safety
//!
//! Loading an arbitrary DLL into a running process is dangerous — a buggy DLL
//! can crash the target, and DLL injection is frequently flagged by antivirus.
//! Use only on processes you own (your own tools, dev environments). Real-game
//! injection needs careful testing against non-production builds first.
//!
//! # Limitations of this PoC
//!
//! - Requires same-user or admin privileges (PROCESS_ALL_ACCESS denied otherwise).
//! - 32-bit/64-bit mismatch between injector and target is not handled.
//! - No error recovery if the DLL's `DllMain` itself fails or panics.
//! - Logs go to the target process's `%TEMP%` (via the DLL's `DllMain`).

use std::env::args;
use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows::core::PCSTR;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Diagnostics::Debug::WriteProcessMemory;
use windows::Win32::System::LibraryLoader::{GetModuleHandleW, GetProcAddress};
use windows::Win32::System::Memory::{
    VirtualAllocEx, VirtualFreeEx, MEM_COMMIT, MEM_RELEASE, MEM_RESERVE, PAGE_READWRITE,
};
use windows::Win32::System::Threading::{
    CreateRemoteThread, OpenProcess, WaitForSingleObject, PROCESS_ALL_ACCESS,
};

fn main() {
    let args: Vec<String> = args().collect();
    if args.len() != 3 {
        eprintln!("Usage: {} <pid> <dll-path>", args[0]);
        std::process::exit(2);
    }

    let pid: u32 = match args[1].parse() {
        Ok(p) => p,
        Err(_) => {
            eprintln!("Invalid PID: {}", args[1]);
            std::process::exit(2);
        }
    };

    let dll_path = PathBuf::from(&args[2]);
    if !dll_path.exists() {
        eprintln!("DLL not found: {}", dll_path.display());
        std::process::exit(2);
    }

    if let Err(e) = inject(pid, &dll_path) {
        eprintln!("[-] Injection failed: {}", e);
        std::process::exit(1);
    }

    println!("[+] Injected into PID {}: {}", pid, dll_path.display());
}

fn inject(pid: u32, dll_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    // 1. Encode DLL path as UTF-16 for LoadLibraryW.
    let mut dll_path_w: Vec<u16> = dll_path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0)) // NUL terminator
        .collect();
    let path_byte_len = dll_path_w.len() * std::mem::size_of::<u16>();

    unsafe {
        // 2. Open target process.
        let process_handle = OpenProcess(PROCESS_ALL_ACCESS, false, pid)
            .map_err(|e| format!("OpenProcess({}): {}", pid, e))?;
        if process_handle.is_invalid() {
            CloseHandle(process_handle).ok();
            return Err(format!("OpenProcess({}) returned invalid handle", pid).into());
        }

        // 3. Allocate memory in target for the path string.
        let remote_mem = VirtualAllocEx(
            process_handle,
            None,
            path_byte_len,
            MEM_COMMIT | MEM_RESERVE,
            PAGE_READWRITE,
        );
        if remote_mem.is_null() {
            CloseHandle(process_handle).ok();
            return Err("VirtualAllocEx returned NULL".into());
        }

        // 4. Write path to target memory.
        if let Err(e) = WriteProcessMemory(
            process_handle,
            remote_mem,
            dll_path_w.as_ptr() as *const c_void,
            path_byte_len,
            None,
        ) {
            let _ = VirtualFreeEx(process_handle, remote_mem, 0, MEM_RELEASE);
            let _ = CloseHandle(process_handle);
            return Err(format!("WriteProcessMemory: {}", e).into());
        }

        // Path string is now in target memory; we can drop our copy.
        dll_path_w.clear();
        dll_path_w.shrink_to_fit();

        // 5. Resolve LoadLibraryW from kernel32 in our own process.
        //    (kernel32.dll's base address is the same in every Windows process
        //    for a given boot session, so the function pointer we read is valid
        //    in the target too.)
        let kernel32_w = wide_null("kernel32.dll");
        let kernel32_handle = GetModuleHandleW(windows::core::PCWSTR(kernel32_w.as_ptr()))
            .map_err(|e| format!("GetModuleHandleW(kernel32.dll): {}", e))?;
        let proc_name = PCSTR(b"LoadLibraryW\0".as_ptr());
        let load_library_w_addr = GetProcAddress(kernel32_handle, proc_name)
            .ok_or_else(|| "GetProcAddress(LoadLibraryW) returned None")?;
        let load_library_w_addr = load_library_w_addr as usize;

        // 6. Cast LoadLibraryW to a thread-proc signature.
        //    Both signatures take 1 pointer-sized arg; we abuse the cast because
        //    CreateRemoteThread's LPTHREAD_START_ROUTINE expects fn(LPVOID)->DWORD
        //    while LoadLibraryW is fn(LPCWSTR)->HMODULE. In practice this works
        //    — the pointer is passed identically in rcx, and the thread's exit
        //    code (DWORD) is read from low 32 bits of rax (we don't check it).
        let thread_proc: unsafe extern "system" fn(*mut c_void) -> u32 =
            std::mem::transmute(load_library_w_addr);

        // 7. Start the remote thread.
        let thread_handle: HANDLE = match CreateRemoteThread(
            process_handle,
            None,
            0,
            Some(thread_proc),
            Some(remote_mem),
            0,
            None,
        ) {
            Ok(h) => h,
            Err(e) => {
                let _ = VirtualFreeEx(process_handle, remote_mem, 0, MEM_RELEASE);
                let _ = CloseHandle(process_handle);
                return Err(format!("CreateRemoteThread: {}", e).into());
            }
        };

        // 8. Wait up to 5 seconds for LoadLibraryW to return.
        //    A healthy DLL load is milliseconds; a hung DllMain could deadlock.
        let _ = WaitForSingleObject(thread_handle, 5000);

        // 9. Cleanup.
        let _ = CloseHandle(thread_handle);
        let _ = VirtualFreeEx(process_handle, remote_mem, 0, MEM_RELEASE);
        let _ = CloseHandle(process_handle);

        Ok(())
    }
}

/// Encode `s` as UTF-16 with a NUL terminator.
fn wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

// Silence unused warning on HANDLE — kept in scope for documentation/future use.
#[allow(dead_code)]
fn _typecheck(_: HANDLE) {}