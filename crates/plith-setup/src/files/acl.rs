//! Who may change the installed files: a port of `Get-AclProblems` from
//! `scripts/install-osd.ps1`. The owner and every allow entry that carries a write right must
//! belong to SYSTEM, Administrators, `TrustedInstaller` or `CREATOR OWNER`.

use std::ffi::c_void;
use std::path::Path;
use windows::Win32::Foundation::{ERROR_SUCCESS, HLOCAL, LocalFree};
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, GetNamedSecurityInfoW, SE_FILE_OBJECT,
};
use windows::Win32::Security::{
    ACE_HEADER, ACL, ACL_SIZE_INFORMATION, AclSizeInformation, DACL_SECURITY_INFORMATION, GetAce,
    GetAclInformation, OWNER_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR, PSID,
};
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{FOLDERID_ProgramFiles, KF_FLAG_DEFAULT, SHGetKnownFolderPath};
use windows::core::{HSTRING, PWSTR};

/// SYSTEM, Administrators, `TrustedInstaller`.
const ADMIN_SIDS: [&str; 3] = [
    "S-1-5-18",
    "S-1-5-32-544",
    "S-1-5-80-956008885-3418522649-1831038044-1853292631-2271478464",
];
/// CREATOR OWNER: it maps to the owner of a new child, and owners are checked separately.
const CREATOR_OWNER: &str = "S-1-3-0";

/// Rights that let a holder change what runs: write or append data (add a file or subfolder),
/// write extended attributes or attributes, delete child, delete, `WRITE_DAC`, `WRITE_OWNER`,
/// `GENERIC_ALL` and `GENERIC_WRITE`.
pub const WRITE_MASK: u32 =
    0x2 | 0x4 | 0x10 | 0x40 | 0x100 | 0x1_0000 | 0x4_0000 | 0x8_0000 | 0x1000_0000 | 0x4000_0000;

/// One entry of a DACL as the check sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ace {
    /// A grant: the SID's text and the rights mask.
    Allowed(String, u32),
    /// An entry type the check does not read; it counts as a problem (fail closed).
    Unknown(u8),
}

/// Judges an owner and the entries of a DACL; one text per problem.
pub fn judge(path: &Path, owner: &str, aces: &[Ace]) -> Vec<String> {
    let mut problems = Vec::new();
    if !ADMIN_SIDS.contains(&owner) {
        problems.push(format!("{} is owned by {owner}", path.display()));
    }
    for ace in aces {
        let (sid, rights) = match ace {
            Ace::Allowed(sid, rights) => (sid, rights),
            Ace::Unknown(kind) => {
                problems.push(format!("{}: unknown ACE type {kind}", path.display()));
                continue;
            }
        };
        if ADMIN_SIDS.contains(&sid.as_str()) || sid == CREATOR_OWNER {
            continue;
        }
        if rights & WRITE_MASK != 0 {
            problems.push(format!(
                "{}: {sid} has rights 0x{rights:08X}",
                path.display()
            ));
        }
    }
    problems
}

/// True when `path` lies under the real `Program Files` folder.
pub fn under_program_files(path: &Path) -> bool {
    // SAFETY: the returned string is copied and then freed with `CoTaskMemFree` once.
    let known = unsafe {
        SHGetKnownFolderPath(&FOLDERID_ProgramFiles, KF_FLAG_DEFAULT, None)
            .ok()
            .map(|folder| {
                let text = folder.to_string().unwrap_or_default();
                CoTaskMemFree(Some(folder.as_ptr().cast()));
                text
            })
    };
    let Some(known) = known.filter(|text| !text.is_empty()) else {
        return false;
    };
    let prefix = format!("{}\\", known.trim_end_matches('\\').to_ascii_lowercase());
    let text = path
        .to_string_lossy()
        .to_ascii_lowercase()
        .replace('/', "\\");
    text.starts_with(&prefix)
}

/// Reads the owner and DACL of `path` and judges them.
///
/// # Errors
/// Windows could not read the security descriptor.
pub fn problems(path: &Path) -> Result<Vec<String>, String> {
    let name = HSTRING::from(path.as_os_str());
    let mut owner = PSID::default();
    let mut dacl: *mut ACL = std::ptr::null_mut();
    let mut descriptor = PSECURITY_DESCRIPTOR::default();
    // SAFETY: the out pointers are valid for the call; the descriptor it allocates owns `owner`
    // and `dacl` and is freed below, after everything is copied out.
    let result = unsafe {
        let status = GetNamedSecurityInfoW(
            &name,
            SE_FILE_OBJECT,
            OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION,
            Some(&raw mut owner),
            None,
            Some(&raw mut dacl),
            None,
            &raw mut descriptor,
        );
        if status == ERROR_SUCCESS {
            read(path, owner, dacl)
        } else {
            Err(format!(
                "cannot read the permissions of {}: error {}",
                path.display(),
                status.0
            ))
        }
    };
    // SAFETY: `descriptor` came from `GetNamedSecurityInfoW` and is freed once.
    unsafe {
        if !descriptor.0.is_null() {
            let _ = LocalFree(Some(HLOCAL(descriptor.0)));
        }
    }
    result
}

/// Copies the owner and the allow entries out of the descriptor and judges them.
///
/// # Safety
/// `owner` and `dacl` must belong to a live security descriptor.
unsafe fn read(path: &Path, owner: PSID, dacl: *mut ACL) -> Result<Vec<String>, String> {
    // SAFETY: `owner` is valid by this function's contract.
    let owner = unsafe { sid_text(owner)? };
    if dacl.is_null() {
        // A missing DACL grants everyone full control.
        return Ok(vec![format!(
            "{} has no DACL (everyone can write)",
            path.display()
        )]);
    }
    let mut info = ACL_SIZE_INFORMATION::default();
    // SAFETY: `info` is the size the call is told; `dacl` is live by the caller's contract.
    unsafe {
        GetAclInformation(
            dacl,
            (&raw mut info).cast::<c_void>(),
            u32::try_from(size_of::<ACL_SIZE_INFORMATION>()).unwrap_or(0),
            AclSizeInformation,
        )
        .map_err(|error| format!("cannot read the ACL of {}: {error}", path.display()))?;
    }
    let mut allowed = Vec::new();
    for index in 0..info.AceCount {
        let mut ace: *mut c_void = std::ptr::null_mut();
        // SAFETY: `index` is below the entry count; the entry stays valid with the descriptor.
        unsafe {
            GetAce(dacl, index, &raw mut ace)
                .map_err(|error| format!("cannot read an ACE of {}: {error}", path.display()))?;
            if let Some(entry) = read_ace(ace)? {
                allowed.push(entry);
            }
        }
    }
    Ok(judge(path, &owner, &allowed))
}

/// Reads one entry; `None` for a deny entry, which grants nothing.
///
/// Allow entries (plain, callback, object, callback object) give their SID and mask. Every other
/// type is `Unknown`, so an entry this check cannot read never passes as harmless.
///
/// # Safety
/// `ace` must point at a live ACE of a DACL.
unsafe fn read_ace(ace: *mut c_void) -> Result<Option<Ace>, String> {
    // SAFETY: `ace` is live by the caller's contract; object entries are read by their
    // documented layout (header, mask, flags, optional two GUIDs, SID), unaligned.
    unsafe {
        let kind = (*ace.cast::<ACE_HEADER>()).AceType;
        let bytes = ace.cast::<u8>();
        let sid_at = match kind {
            // Plain and callback allow: header, mask, SID.
            0 | 9 => 8,
            // Object and callback object allow: header, mask, flags, the GUIDs the flags name.
            5 | 11 => {
                let flags = bytes.add(8).cast::<u32>().read_unaligned();
                12 + 16 * (flags & 1) as usize + 16 * ((flags >> 1) & 1) as usize
            }
            // Deny entries (plain, object, callback, callback object).
            1 | 6 | 10 | 12 => return Ok(None),
            other => return Ok(Some(Ace::Unknown(other))),
        };
        let mask = bytes.add(4).cast::<u32>().read_unaligned();
        let sid = PSID(bytes.add(sid_at).cast());
        Ok(Some(Ace::Allowed(sid_text(sid)?, mask)))
    }
}

/// A SID as `S-1-...` text.
///
/// # Safety
/// `sid` must point at a valid SID.
unsafe fn sid_text(sid: PSID) -> Result<String, String> {
    let mut text = PWSTR::null();
    // SAFETY: `sid` is valid by the caller's contract; the string is copied, then freed once.
    unsafe {
        ConvertSidToStringSidW(sid, &raw mut text)
            .map_err(|error| format!("cannot read a SID: {error}"))?;
        let copy = text.to_string().unwrap_or_default();
        let _ = LocalFree(Some(HLOCAL(text.0.cast())));
        Ok(copy)
    }
}
