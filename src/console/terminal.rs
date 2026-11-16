use std::{
    env,
    ffi::OsStr,
    io,
    os::windows::{
        ffi::OsStrExt,
        io::{FromRawHandle, OwnedHandle},
    },
    path::Path,
    ptr::null,
};

use windows_sys::Win32::{
    Foundation::CloseHandle,
    System::Threading::{
        CreateProcessW, CREATE_NEW_CONSOLE, CREATE_UNICODE_ENVIRONMENT, PROCESS_INFORMATION,
        STARTUPINFOW,
    },
};

pub(super) fn spawn(executable: &Path, argv: &[String]) -> io::Result<OwnedHandle> {
    if !executable.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "CLI path must be absolute",
        ));
    }
    let executable: Vec<_> = executable.as_os_str().encode_wide().collect();
    let mut command_line = Vec::new();
    append_argument(&mut command_line, executable.iter().copied())?;
    for argument in argv {
        command_line.push(u16::from(b' '));
        append_argument(&mut command_line, argument.encode_utf16())?;
    }
    command_line.push(0);
    let application: Vec<_> = executable.into_iter().chain([0]).collect();
    let environment = terminal_environment();
    let startup = STARTUPINFOW {
        cb: std::mem::size_of::<STARTUPINFOW>() as u32,
        ..Default::default()
    };
    let mut process = PROCESS_INFORMATION::default();

    // With no STARTF_USESTDHANDLES the new console supplies all three handles.
    // CreateProcessW receives an explicit image path, writable argv buffer and
    // a double-NUL terminated Unicode environment.
    let created = unsafe {
        CreateProcessW(
            application.as_ptr(),
            command_line.as_mut_ptr(),
            null(),
            null(),
            0,
            CREATE_NEW_CONSOLE | CREATE_UNICODE_ENVIRONMENT,
            environment.as_ptr().cast(),
            null(),
            &startup,
            &mut process,
        )
    };
    if created == 0 {
        return Err(io::Error::last_os_error());
    }
    // Success returns two owned handles. The thread handle is no longer needed.
    unsafe {
        CloseHandle(process.hThread);
        Ok(OwnedHandle::from_raw_handle(process.hProcess))
    }
}

fn terminal_environment() -> Vec<u16> {
    let mut entries: Vec<_> = env::vars_os()
        .filter(|(name, _)| {
            !["OXVIF_USERNAME", "OXVIF_PASSWORD", "OXVIF_DEVICE"]
                .iter()
                .any(|secret| name.eq_ignore_ascii_case(OsStr::new(secret)))
        })
        .collect();
    entries.sort_unstable_by_key(|(name, _)| name.to_string_lossy().to_uppercase());
    let mut block = Vec::new();
    for (name, value) in entries {
        block.extend(name.encode_wide());
        block.push(u16::from(b'='));
        block.extend(value.encode_wide());
        block.push(0);
    }
    if block.is_empty() {
        block.push(0);
    }
    block.push(0);
    block
}

fn append_argument(buffer: &mut Vec<u16>, units: impl IntoIterator<Item = u16>) -> io::Result<()> {
    const QUOTE: u16 = b'"' as u16;
    const BACKSLASH: u16 = b'\\' as u16;
    buffer.push(QUOTE);
    let mut backslashes = 0;
    for unit in units {
        match unit {
            0 => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "Argument contains NUL",
                ))
            }
            BACKSLASH => backslashes += 1,
            _ => {
                let count = if unit == QUOTE {
                    2 * backslashes + 1
                } else {
                    backslashes
                };
                buffer.extend(std::iter::repeat_n(BACKSLASH, count));
                buffer.push(unit);
                backslashes = 0;
            }
        }
    }
    buffer.extend(std::iter::repeat_n(BACKSLASH, 2 * backslashes));
    buffer.push(QUOTE);
    Ok(())
}
