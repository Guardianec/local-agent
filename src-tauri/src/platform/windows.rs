use crate::domain::{AppError, Result, RootIdentity};
use std::{
    fs::{File, OpenOptions},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::Path,
};
use windows::{
    core::PCWSTR,
    Win32::{
        Foundation::{LocalFree, HANDLE, HLOCAL},
        Security::Cryptography::*,
        Storage::FileSystem::*,
    },
};

pub fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(Some(0)).collect()
}
pub fn lock_instance(data: &Path) -> Result<File> {
    std::fs::create_dir_all(data)?;
    OpenOptions::new().write(true).create(true).truncate(false).share_mode(0)
        .open(data.join("instance.lock"))
        .map_err(|error| AppError::new("APP_ALREADY_OPEN", "Local Agent is already open, or its data folder is locked. Close the other window before reopening it.").detail(error))
}
pub fn identity(file: &File) -> Result<RootIdentity> {
    let info = file_info(file)?;
    Ok(RootIdentity {
        volume_id: info.dwVolumeSerialNumber.to_string(),
        file_id: format!("{}:{}", info.nFileIndexHigh, info.nFileIndexLow),
    })
}
pub fn file_info(file: &File) -> Result<BY_HANDLE_FILE_INFORMATION> {
    let mut info = BY_HANDLE_FILE_INFORMATION::default();
    unsafe { GetFileInformationByHandle(HANDLE(file.as_raw_handle()), &mut info) }.map_err(
        |e| AppError::new("FILE_IDENTITY", "The file identity could not be verified.").detail(e),
    )?;
    Ok(info)
}
pub fn open_dir(path: &Path) -> Result<File> {
    Ok(OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ.0 | FILE_SHARE_WRITE.0)
        .custom_flags(FILE_FLAG_BACKUP_SEMANTICS.0 | FILE_FLAG_OPEN_REPARSE_POINT.0)
        .open(path)?)
}
pub fn open_file(path: &Path, write: bool, create: bool) -> Result<File> {
    let mut options = OpenOptions::new();
    options
        .read(true)
        .write(write)
        .share_mode(if write { 0 } else { FILE_SHARE_READ.0 })
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT.0);
    if write {
        options.access_mode(FILE_GENERIC_READ.0 | FILE_GENERIC_WRITE.0 | DELETE.0);
    }
    if create {
        options.create_new(true);
    }
    let file = options.open(path)?;
    let info = file_info(&file)?;
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0
        || (write && info.nNumberOfLinks > 1)
    {
        return Err(AppError::new(
            "UNSAFE_LINK",
            "Linked files cannot be changed. Choose an ordinary file.",
        ));
    }
    Ok(file)
}
pub fn ensure_volume(path: &Path) -> Result<()> {
    let s = path.to_string_lossy();
    let drive = if let Some(s) = s.strip_prefix(r"\\?\") {
        s
    } else {
        &s
    };
    if drive.len() < 3 || drive.as_bytes().get(1) != Some(&b':') {
        return Err(AppError::new(
            "UNSUPPORTED_FOLDER",
            "Choose a local folder on a fixed NTFS drive.",
        ));
    }
    let root = wide(std::ffi::OsStr::new(&drive[..3]));
    let mut fs_name = [0u16; 64];
    let kind = unsafe { GetDriveTypeW(PCWSTR(root.as_ptr())) };
    unsafe {
        GetVolumeInformationW(
            PCWSTR(root.as_ptr()),
            None,
            None,
            None,
            None,
            Some(&mut fs_name),
        )
    }
    .map_err(|e| AppError::new("VOLUME", "This project drive could not be checked.").detail(e))?;
    if kind != 3 || String::from_utf16_lossy(&fs_name).trim_end_matches('\0') != "NTFS" {
        return Err(AppError::new(
            "UNSUPPORTED_FOLDER",
            "Choose a local folder on a fixed NTFS drive.",
        ));
    }
    Ok(())
}
pub fn ensure_ordinary(file: &File) -> Result<()> {
    let info = file_info(file)?;
    if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT.0 != 0 {
        return Err(AppError::new(
            "UNSAFE_LINK",
            "Links and cloud placeholder folders are excluded.",
        ));
    }
    // Windows case-sensitive directories are deliberately outside the supported envelope.
    #[repr(C)]
    #[derive(Default)]
    struct CaseSensitiveInfo {
        flags: u32,
    }
    let mut case = CaseSensitiveInfo::default();
    let result = unsafe {
        GetFileInformationByHandleEx(
            HANDLE(file.as_raw_handle()),
            FileCaseSensitiveInfo,
            &mut case as *mut _ as *mut _,
            std::mem::size_of_val(&case) as u32,
        )
    };
    if result.is_ok() && case.flags & 1 != 0 {
        return Err(AppError::new(
            "CASE_SENSITIVE",
            "Choose a folder using ordinary Windows file names.",
        ));
    }
    Ok(())
}
pub fn free_bytes(path: &Path) -> Result<u64> {
    let path = wide(path.as_os_str());
    let mut available = 0u64;
    unsafe { GetDiskFreeSpaceExW(PCWSTR(path.as_ptr()), Some(&mut available), None, None) }
        .map_err(|e| {
            AppError::new(
                "DISK_SPACE",
                "Available recovery storage could not be checked.",
            )
            .detail(e)
        })?;
    Ok(available)
}
pub fn seal(bytes: &[u8]) -> Result<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptProtectData(
            &input,
            PCWSTR::null(),
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|e| {
            AppError::new("ENCRYPTION", "Recovery data could not be protected.").detail(e)
        })?;
        let data = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(output.pbData as *mut _));
        Ok(data)
    }
}
pub fn unseal(bytes: &[u8]) -> Result<Vec<u8>> {
    let input = CRYPT_INTEGER_BLOB {
        cbData: bytes.len() as u32,
        pbData: bytes.as_ptr() as *mut u8,
    };
    let mut output = CRYPT_INTEGER_BLOB::default();
    unsafe {
        CryptUnprotectData(
            &input,
            None,
            None,
            None,
            None,
            CRYPTPROTECT_UI_FORBIDDEN,
            &mut output,
        )
        .map_err(|e| {
            AppError::new(
                "RECOVERY_KEY",
                "Windows could not unlock this recovery data for your account.",
            )
            .detail(e)
        })?;
        let data = std::slice::from_raw_parts(output.pbData, output.cbData as usize).to_vec();
        let _ = LocalFree(HLOCAL(output.pbData as *mut _));
        Ok(data)
    }
}
