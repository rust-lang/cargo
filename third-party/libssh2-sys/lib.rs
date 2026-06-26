#![doc(html_root_url = "http://alexcrichton.com/ssh2-rs")]
#![allow(bad_style)]
#![allow(unused_extern_crates)]

extern crate libc;

extern crate libz_sys;
#[cfg(unix)]
extern crate openssl_sys;

use libc::ssize_t;
use libc::{c_char, c_int, c_long, c_uchar, c_uint, c_ulong, c_void, size_t};

pub const SSH_DISCONNECT_HOST_NOT_ALLOWED_TO_CONNECT: c_int = 1;
pub const SSH_DISCONNECT_PROTOCOL_ERROR: c_int = 2;
pub const SSH_DISCONNECT_KEY_EXCHANGE_FAILED: c_int = 3;
pub const SSH_DISCONNECT_RESERVED: c_int = 4;
pub const SSH_DISCONNECT_MAC_ERROR: c_int = 5;
pub const SSH_DISCONNECT_COMPRESSION_ERROR: c_int = 6;
pub const SSH_DISCONNECT_SERVICE_NOT_AVAILABLE: c_int = 7;
pub const SSH_DISCONNECT_PROTOCOL_VERSION_NOT_SUPPORTED: c_int = 8;
pub const SSH_DISCONNECT_HOST_KEY_NOT_VERIFIABLE: c_int = 9;
pub const SSH_DISCONNECT_CONNECTION_LOST: c_int = 10;
pub const SSH_DISCONNECT_BY_APPLICATION: c_int = 11;
pub const SSH_DISCONNECT_TOO_MANY_CONNECTIONS: c_int = 12;
pub const SSH_DISCONNECT_AUTH_CANCELLED_BY_USER: c_int = 13;
pub const SSH_DISCONNECT_NO_MORE_AUTH_METHODS_AVAILABLE: c_int = 14;
pub const SSH_DISCONNECT_ILLEGAL_USER_NAME: c_int = 15;

pub const LIBSSH2_FLAG_SIGPIPE: c_int = 1;
pub const LIBSSH2_FLAG_COMPRESS: c_int = 2;

pub const LIBSSH2_HOSTKEY_TYPE_UNKNOWN: c_int = 0;
pub const LIBSSH2_HOSTKEY_TYPE_RSA: c_int = 1;
pub const LIBSSH2_HOSTKEY_TYPE_DSS: c_int = 2;
pub const LIBSSH2_HOSTKEY_TYPE_ECDSA_256: c_int = 3;
pub const LIBSSH2_HOSTKEY_TYPE_ECDSA_384: c_int = 4;
pub const LIBSSH2_HOSTKEY_TYPE_ECDSA_521: c_int = 5;
pub const LIBSSH2_HOSTKEY_TYPE_ED25519: c_int = 6;

pub const LIBSSH2_METHOD_KEX: c_int = 0;
pub const LIBSSH2_METHOD_HOSTKEY: c_int = 1;
pub const LIBSSH2_METHOD_CRYPT_CS: c_int = 2;
pub const LIBSSH2_METHOD_CRYPT_SC: c_int = 3;
pub const LIBSSH2_METHOD_MAC_CS: c_int = 4;
pub const LIBSSH2_METHOD_MAC_SC: c_int = 5;
pub const LIBSSH2_METHOD_COMP_CS: c_int = 6;
pub const LIBSSH2_METHOD_COMP_SC: c_int = 7;
pub const LIBSSH2_METHOD_LANG_CS: c_int = 8;
pub const LIBSSH2_METHOD_LANG_SC: c_int = 9;
pub const LIBSSH2_METHOD_SIGN_ALGO: c_int = 10;

pub const LIBSSH2_CHANNEL_PACKET_DEFAULT: c_uint = 32768;
pub const LIBSSH2_CHANNEL_WINDOW_DEFAULT: c_uint = 2 * 1024 * 1024;

pub const LIBSSH2_ERROR_BANNER_RECV: c_int = -2;
pub const LIBSSH2_ERROR_BANNER_SEND: c_int = -3;
pub const LIBSSH2_ERROR_INVALID_MAC: c_int = -4;
pub const LIBSSH2_ERROR_KEX_FAILURE: c_int = -5;
pub const LIBSSH2_ERROR_ALLOC: c_int = -6;
pub const LIBSSH2_ERROR_SOCKET_SEND: c_int = -7;
pub const LIBSSH2_ERROR_KEY_EXCHANGE_FAILURE: c_int = -8;
pub const LIBSSH2_ERROR_TIMEOUT: c_int = -9;
pub const LIBSSH2_ERROR_HOSTKEY_INIT: c_int = -10;
pub const LIBSSH2_ERROR_HOSTKEY_SIGN: c_int = -11;
pub const LIBSSH2_ERROR_DECRYPT: c_int = -12;
pub const LIBSSH2_ERROR_SOCKET_DISCONNECT: c_int = -13;
pub const LIBSSH2_ERROR_PROTO: c_int = -14;
pub const LIBSSH2_ERROR_PASSWORD_EXPIRED: c_int = -15;
pub const LIBSSH2_ERROR_FILE: c_int = -16;
pub const LIBSSH2_ERROR_METHOD_NONE: c_int = -17;
pub const LIBSSH2_ERROR_AUTHENTICATION_FAILED: c_int = -18;
pub const LIBSSH2_ERROR_PUBLICKEY_UNRECOGNIZED: c_int = LIBSSH2_ERROR_AUTHENTICATION_FAILED;
pub const LIBSSH2_ERROR_PUBLICKEY_UNVERIFIED: c_int = -19;
pub const LIBSSH2_ERROR_CHANNEL_OUTOFORDER: c_int = -20;
pub const LIBSSH2_ERROR_CHANNEL_FAILURE: c_int = -21;
pub const LIBSSH2_ERROR_CHANNEL_REQUEST_DENIED: c_int = -22;
pub const LIBSSH2_ERROR_CHANNEL_UNKNOWN: c_int = -23;
pub const LIBSSH2_ERROR_CHANNEL_WINDOW_EXCEEDED: c_int = -24;
pub const LIBSSH2_ERROR_CHANNEL_PACKET_EXCEEDED: c_int = -25;
pub const LIBSSH2_ERROR_CHANNEL_CLOSED: c_int = -26;
pub const LIBSSH2_ERROR_CHANNEL_EOF_SENT: c_int = -27;
pub const LIBSSH2_ERROR_SCP_PROTOCOL: c_int = -28;
pub const LIBSSH2_ERROR_ZLIB: c_int = -29;
pub const LIBSSH2_ERROR_SOCKET_TIMEOUT: c_int = -30;
pub const LIBSSH2_ERROR_SFTP_PROTOCOL: c_int = -31;
pub const LIBSSH2_ERROR_REQUEST_DENIED: c_int = -32;
pub const LIBSSH2_ERROR_METHOD_NOT_SUPPORTED: c_int = -33;
pub const LIBSSH2_ERROR_INVAL: c_int = -34;
pub const LIBSSH2_ERROR_INVALID_POLL_TYPE: c_int = -35;
pub const LIBSSH2_ERROR_PUBLICKEY_PROTOCOL: c_int = -36;
pub const LIBSSH2_ERROR_EAGAIN: c_int = -37;
pub const LIBSSH2_ERROR_BUFFER_TOO_SMALL: c_int = -38;
pub const LIBSSH2_ERROR_BAD_USE: c_int = -39;
pub const LIBSSH2_ERROR_COMPRESS: c_int = -40;
pub const LIBSSH2_ERROR_OUT_OF_BOUNDARY: c_int = -41;
pub const LIBSSH2_ERROR_AGENT_PROTOCOL: c_int = -42;
pub const LIBSSH2_ERROR_SOCKET_RECV: c_int = -43;
pub const LIBSSH2_ERROR_ENCRYPT: c_int = -44;
pub const LIBSSH2_ERROR_BAD_SOCKET: c_int = -45;
pub const LIBSSH2_ERROR_KNOWN_HOSTS: c_int = -46;
pub const LIBSSH2_ERROR_CHANNEL_WINDOW_FULL: c_int = -47;
pub const LIBSSH2_ERROR_KEYFILE_AUTH_FAILED: c_int = -48;
pub const LIBSSH2_ERROR_RANDGEN: c_int = -49;
pub const LIBSSH2_ERROR_MISSING_USERAUTH_BANNER: c_int = -50;
pub const LIBSSH2_ERROR_ALGO_UNSUPPORTED: c_int = -51;

pub const LIBSSH2_FX_EOF: c_int = 1;
pub const LIBSSH2_FX_NO_SUCH_FILE: c_int = 2;
pub const LIBSSH2_FX_PERMISSION_DENIED: c_int = 3;
pub const LIBSSH2_FX_FAILURE: c_int = 4;
pub const LIBSSH2_FX_BAD_MESSAGE: c_int = 5;
pub const LIBSSH2_FX_NO_CONNECTION: c_int = 6;
pub const LIBSSH2_FX_CONNECTION_LOST: c_int = 7;
pub const LIBSSH2_FX_OP_UNSUPPORTED: c_int = 8;
pub const LIBSSH2_FX_INVALID_HANDLE: c_int = 9;
pub const LIBSSH2_FX_NO_SUCH_PATH: c_int = 10;
pub const LIBSSH2_FX_FILE_ALREADY_EXISTS: c_int = 11;
pub const LIBSSH2_FX_WRITE_PROTECT: c_int = 12;
pub const LIBSSH2_FX_NO_MEDIA: c_int = 13;
pub const LIBSSH2_FX_NO_SPACE_ON_FILESYSTEM: c_int = 14;
pub const LIBSSH2_FX_QUOTA_EXCEEDED: c_int = 15;
pub const LIBSSH2_FX_UNKNOWN_PRINCIPAL: c_int = 16;
pub const LIBSSH2_FX_LOCK_CONFLICT: c_int = 17;
pub const LIBSSH2_FX_DIR_NOT_EMPTY: c_int = 18;
pub const LIBSSH2_FX_NOT_A_DIRECTORY: c_int = 19;
pub const LIBSSH2_FX_INVALID_FILENAME: c_int = 20;
pub const LIBSSH2_FX_LINK_LOOP: c_int = 21;

pub const LIBSSH2_HOSTKEY_HASH_MD5: c_int = 1;
pub const LIBSSH2_HOSTKEY_HASH_SHA1: c_int = 2;
pub const LIBSSH2_HOSTKEY_HASH_SHA256: c_int = 3;

pub const LIBSSH2_KNOWNHOST_FILE_OPENSSH: c_int = 1;

pub const LIBSSH2_KNOWNHOST_CHECK_MATCH: c_int = 0;
pub const LIBSSH2_KNOWNHOST_CHECK_MISMATCH: c_int = 1;
pub const LIBSSH2_KNOWNHOST_CHECK_NOTFOUND: c_int = 2;
pub const LIBSSH2_KNOWNHOST_CHECK_FAILURE: c_int = 3;

pub const LIBSSH2_KNOWNHOST_TYPE_PLAIN: c_int = 1;
pub const LIBSSH2_KNOWNHOST_TYPE_SHA1: c_int = 2;
pub const LIBSSH2_KNOWNHOST_TYPE_CUSTOM: c_int = 3;
pub const LIBSSH2_KNOWNHOST_KEYENC_RAW: c_int = 1 << 16;
pub const LIBSSH2_KNOWNHOST_KEYENC_BASE64: c_int = 2 << 16;
pub const LIBSSH2_KNOWNHOST_KEY_RSA1: c_int = 1 << 18;
pub const LIBSSH2_KNOWNHOST_KEY_SSHRSA: c_int = 2 << 18;
pub const LIBSSH2_KNOWNHOST_KEY_SSHDSS: c_int = 3 << 18;
pub const LIBSSH2_KNOWNHOST_KEY_ECDSA_256: c_int = 4 << 18;
pub const LIBSSH2_KNOWNHOST_KEY_ECDSA_384: c_int = 5 << 18;
pub const LIBSSH2_KNOWNHOST_KEY_ECDSA_521: c_int = 6 << 18;
pub const LIBSSH2_KNOWNHOST_KEY_ED25519: c_int = 7 << 18;
pub const LIBSSH2_KNOWNHOST_KEY_UNKNOWN: c_int = 15 << 18;

pub const LIBSSH2_FXF_READ: c_ulong = 0x00000001;
pub const LIBSSH2_FXF_WRITE: c_ulong = 0x00000002;
pub const LIBSSH2_FXF_APPEND: c_ulong = 0x00000004;
pub const LIBSSH2_FXF_CREAT: c_ulong = 0x00000008;
pub const LIBSSH2_FXF_TRUNC: c_ulong = 0x00000010;
pub const LIBSSH2_FXF_EXCL: c_ulong = 0x00000020;

pub const LIBSSH2_SFTP_OPENFILE: c_int = 0;
pub const LIBSSH2_SFTP_OPENDIR: c_int = 1;

pub const LIBSSH2_SFTP_ATTR_SIZE: c_ulong = 0x00000001;
pub const LIBSSH2_SFTP_ATTR_UIDGID: c_ulong = 0x00000002;
pub const LIBSSH2_SFTP_ATTR_PERMISSIONS: c_ulong = 0x00000004;
pub const LIBSSH2_SFTP_ATTR_ACMODTIME: c_ulong = 0x00000008;
pub const LIBSSH2_SFTP_ATTR_EXTENDED: c_ulong = 0x80000000;

pub const LIBSSH2_SFTP_STAT: c_int = 0;
pub const LIBSSH2_SFTP_LSTAT: c_int = 1;
pub const LIBSSH2_SFTP_SETSTAT: c_int = 2;

pub const LIBSSH2_SFTP_SYMLINK: c_int = 0;
pub const LIBSSH2_SFTP_READLINK: c_int = 1;
pub const LIBSSH2_SFTP_REALPATH: c_int = 2;

pub const LIBSSH2_SFTP_RENAME_OVERWRITE: c_long = 0x1;
pub const LIBSSH2_SFTP_RENAME_ATOMIC: c_long = 0x2;
pub const LIBSSH2_SFTP_RENAME_NATIVE: c_long = 0x4;

pub const LIBSSH2_INIT_NO_CRYPTO: c_int = 0x1;

pub const LIBSSH2_SFTP_S_IFMT: c_ulong = 0o170000;
pub const LIBSSH2_SFTP_S_IFIFO: c_ulong = 0o010000;
pub const LIBSSH2_SFTP_S_IFCHR: c_ulong = 0o020000;
pub const LIBSSH2_SFTP_S_IFDIR: c_ulong = 0o040000;
pub const LIBSSH2_SFTP_S_IFBLK: c_ulong = 0o060000;
pub const LIBSSH2_SFTP_S_IFREG: c_ulong = 0o100000;
pub const LIBSSH2_SFTP_S_IFLNK: c_ulong = 0o120000;
pub const LIBSSH2_SFTP_S_IFSOCK: c_ulong = 0o140000;

pub const LIBSSH2_CHANNEL_EXTENDED_DATA_NORMAL: c_int = 0;
pub const LIBSSH2_CHANNEL_EXTENDED_DATA_IGNORE: c_int = 1;
pub const LIBSSH2_CHANNEL_EXTENDED_DATA_MERGE: c_int = 2;

pub const LIBSSH2_SESSION_BLOCK_INBOUND: c_int = 1;
pub const LIBSSH2_SESSION_BLOCK_OUTBOUND: c_int = 2;

pub const  LIBSSH2_TRACE_TRANS : c_int = 1<<1;
pub const  LIBSSH2_TRACE_KEX   : c_int = 1<<2;
pub const  LIBSSH2_TRACE_AUTH  : c_int = 1<<3;
pub const  LIBSSH2_TRACE_CONN  : c_int = 1<<4;
pub const  LIBSSH2_TRACE_SCP   : c_int = 1<<5;
pub const  LIBSSH2_TRACE_SFTP  : c_int = 1<<6;
pub const  LIBSSH2_TRACE_ERROR : c_int = 1<<7;
pub const  LIBSSH2_TRACE_PUBLICKEY : c_int = 1<<8;
pub const  LIBSSH2_TRACE_SOCKET : c_int = 1<<9;
pub enum LIBSSH2_SESSION {}
pub enum LIBSSH2_AGENT {}
pub enum LIBSSH2_CHANNEL {}
pub enum LIBSSH2_LISTENER {}
pub enum LIBSSH2_KNOWNHOSTS {}
pub enum LIBSSH2_SFTP {}
pub enum LIBSSH2_SFTP_HANDLE {}

pub type libssh2_int64_t = i64;
pub type libssh2_uint64_t = u64;

// libssh2_struct_stat is a typedef for libc::stat on all platforms, however,
// Windows has a bunch of legacy around struct stat that makes things more
// complicated to validate with systest.
// The most reasonable looking solution to this is a newtype that derefs
// to libc::stat.
// We cannot use `pub struct libssh2_struct_stat(pub libc::stat)` because
// that triggers a `no tuple structs in FFI` error.
#[repr(C)]
pub struct libssh2_struct_stat(libc::stat);

impl std::ops::Deref for libssh2_struct_stat {
    type Target = libc::stat;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(C)]
pub struct libssh2_agent_publickey {
    pub magic: c_uint,
    pub node: *mut c_void,
    pub blob: *mut c_uchar,
    pub blob_len: size_t,
    pub comment: *mut c_char,
}

#[repr(C)]
pub struct libssh2_knownhost {
    pub magic: c_uint,
    pub node: *mut c_void,
    pub name: *mut c_char,
    pub key: *mut c_char,
    pub typemask: c_int,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct LIBSSH2_SFTP_ATTRIBUTES {
    pub flags: c_ulong,
    pub filesize: libssh2_uint64_t,
    pub uid: c_ulong,
    pub gid: c_ulong,
    pub permissions: c_ulong,
    pub atime: c_ulong,
    pub mtime: c_ulong,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct LIBSSH2_SFTP_STATVFS {
    pub f_bsize: libssh2_uint64_t,
    pub f_frsize: libssh2_uint64_t,
    pub f_blocks: libssh2_uint64_t,
    pub f_bfree: libssh2_uint64_t,
    pub f_bavail: libssh2_uint64_t,
    pub f_files: libssh2_uint64_t,
    pub f_ffree: libssh2_uint64_t,
    pub f_favail: libssh2_uint64_t,
    pub f_fsid: libssh2_uint64_t,
    pub f_flag: libssh2_uint64_t,
    pub f_namemax: libssh2_uint64_t,
}

pub type LIBSSH2_ALLOC_FUNC = extern "C" fn(size_t, *mut *mut c_void) -> *mut c_void;
pub type LIBSSH2_FREE_FUNC = extern "C" fn(*mut c_void, *mut *mut c_void);
pub type LIBSSH2_REALLOC_FUNC = extern "C" fn(*mut c_void, size_t, *mut *mut c_void) -> *mut c_void;
pub type LIBSSH2_PASSWD_CHANGEREQ_FUNC = extern "C" fn(
    sess: *mut LIBSSH2_SESSION,
    newpw: *mut *mut c_char,
    newpw_len: *mut c_int,
    abstrakt: *mut *mut c_void,
);

pub type LIBSSH2_USERAUTH_KBDINT_RESPONSE_FUNC = extern "C" fn(
    username: *const c_char,
    username_len: c_int,
    instruction: *const c_char,
    instruction_len: c_int,
    num_prompts: c_int,
    prompts: *const LIBSSH2_USERAUTH_KBDINT_PROMPT,
    responses: *mut LIBSSH2_USERAUTH_KBDINT_RESPONSE,
    abstrakt: *mut *mut c_void,
);

#[repr(C)]
pub struct LIBSSH2_USERAUTH_KBDINT_PROMPT {
    pub text: *mut c_uchar,
    pub length: size_t,
    pub echo: c_uchar,
}

#[repr(C)]
pub struct LIBSSH2_USERAUTH_KBDINT_RESPONSE {
    pub text: *mut c_char,
    pub length: c_uint,
}

#[cfg(unix)]
pub type libssh2_socket_t = c_int;
#[cfg(all(windows, target_pointer_width = "32"))]
pub type libssh2_socket_t = u32;
#[cfg(all(windows, target_pointer_width = "64"))]
pub type libssh2_socket_t = u64;

extern "C" {
    // misc
    pub fn libssh2_init(flag: c_int) -> c_int;
    pub fn libssh2_exit();
    pub fn libssh2_free(sess: *mut LIBSSH2_SESSION, ptr: *mut c_void);
    pub fn libssh2_hostkey_hash(session: *mut LIBSSH2_SESSION, hash_type: c_int) -> *const c_char;
    pub fn libssh2_trace(session: *mut LIBSSH2_SESSION, bitmask: c_int) -> c_int;

    // session
    pub fn libssh2_session_init_ex(
        alloc: Option<LIBSSH2_ALLOC_FUNC>,
        free: Option<LIBSSH2_FREE_FUNC>,
        realloc: Option<LIBSSH2_REALLOC_FUNC>,
        abstrakt: *mut c_void,
    ) -> *mut LIBSSH2_SESSION;
    pub fn libssh2_session_abstract(session: *mut LIBSSH2_SESSION) -> *mut *mut c_void;
    pub fn libssh2_session_free(sess: *mut LIBSSH2_SESSION) -> c_int;
    pub fn libssh2_session_banner_get(sess: *mut LIBSSH2_SESSION) -> *const c_char;
    pub fn libssh2_session_banner_set(sess: *mut LIBSSH2_SESSION, banner: *const c_char) -> c_int;
    pub fn libssh2_session_disconnect_ex(
        sess: *mut LIBSSH2_SESSION,
        reason: c_int,
        description: *const c_char,
        lang: *const c_char,
    ) -> c_int;
    pub fn libssh2_session_flag(sess: *mut LIBSSH2_SESSION, flag: c_int, value: c_int) -> c_int;
    pub fn libssh2_session_get_blocking(session: *mut LIBSSH2_SESSION) -> c_int;
    pub fn libssh2_session_get_timeout(sess: *mut LIBSSH2_SESSION) -> c_long;
    pub fn libssh2_session_hostkey(
        sess: *mut LIBSSH2_SESSION,
        len: *mut size_t,
        kind: *mut c_int,
    ) -> *const c_char;
    pub fn libssh2_session_method_pref(
        sess: *mut LIBSSH2_SESSION,
        method_type: c_int,
        prefs: *const c_char,
    ) -> c_int;
    pub fn libssh2_session_methods(sess: *mut LIBSSH2_SESSION, method_type: c_int)
        -> *const c_char;
    pub fn libssh2_session_set_blocking(session: *mut LIBSSH2_SESSION, blocking: c_int);
    pub fn libssh2_session_set_timeout(session: *mut LIBSSH2_SESSION, timeout: c_long);
    pub fn libssh2_session_supported_algs(
        session: *mut LIBSSH2_SESSION,
        method_type: c_int,
        algs: *mut *mut *const c_char,
    ) -> c_int;
    pub fn libssh2_session_last_errno(sess: *mut LIBSSH2_SESSION) -> c_int;
    pub fn libssh2_session_last_error(
        sess: *mut LIBSSH2_SESSION,
        msg: *mut *mut c_char,
        len: *mut c_int,
        want_buf: c_int,
    ) -> c_int;
    pub fn libssh2_session_handshake(sess: *mut LIBSSH2_SESSION, socket: libssh2_socket_t)
        -> c_int;
    pub fn libssh2_keepalive_config(
        sess: *mut LIBSSH2_SESSION,
        want_reply: c_int,
        interval: c_uint,
    );
    pub fn libssh2_keepalive_send(sess: *mut LIBSSH2_SESSION, seconds_to_next: *mut c_int)
        -> c_int;
    pub fn libssh2_session_block_directions(sess: *mut LIBSSH2_SESSION) -> c_int;

    // agent
    pub fn libssh2_agent_init(sess: *mut LIBSSH2_SESSION) -> *mut LIBSSH2_AGENT;
    pub fn libssh2_agent_free(agent: *mut LIBSSH2_AGENT);
    pub fn libssh2_agent_connect(agent: *mut LIBSSH2_AGENT) -> c_int;
    pub fn libssh2_agent_disconnect(agent: *mut LIBSSH2_AGENT) -> c_int;
    pub fn libssh2_agent_list_identities(agent: *mut LIBSSH2_AGENT) -> c_int;
    pub fn libssh2_agent_get_identity(
        agent: *mut LIBSSH2_AGENT,
        store: *mut *mut libssh2_agent_publickey,
        prev: *mut libssh2_agent_publickey,
    ) -> c_int;
    pub fn libssh2_agent_userauth(
        agent: *mut LIBSSH2_AGENT,
        username: *const c_char,
        identity: *mut libssh2_agent_publickey,
    ) -> c_int;

    // channels
    pub fn libssh2_channel_free(chan: *mut LIBSSH2_CHANNEL) -> c_int;
    pub fn libssh2_channel_close(chan: *mut LIBSSH2_CHANNEL) -> c_int;
    pub fn libssh2_channel_wait_closed(chan: *mut LIBSSH2_CHANNEL) -> c_int;
    pub fn libssh2_channel_wait_eof(chan: *mut LIBSSH2_CHANNEL) -> c_int;
    pub fn libssh2_channel_eof(chan: *mut LIBSSH2_CHANNEL) -> c_int;
    pub fn libssh2_channel_process_startup(
        chan: *mut LIBSSH2_CHANNEL,
        req: *const c_char,
        req_len: c_uint,
        msg: *const c_char,
        msg_len: c_uint,
    ) -> c_int;
    pub fn libssh2_channel_flush_ex(chan: *mut LIBSSH2_CHANNEL, streamid: c_int) -> c_int;
    pub fn libssh2_channel_write_ex(
        chan: *mut LIBSSH2_CHANNEL,
        stream_id: c_int,
        buf: *const c_char,
        buflen: size_t,
    ) -> ssize_t;
    pub fn libssh2_channel_get_exit_signal(
        chan: *mut LIBSSH2_CHANNEL,
        exitsignal: *mut *mut c_char,
        exitsignal_len: *mut size_t,
        errmsg: *mut *mut c_char,
        errmsg_len: *mut size_t,
        langtag: *mut *mut c_char,
        langtag_len: *mut size_t,
    ) -> c_int;
    pub fn libssh2_channel_get_exit_status(chan: *mut LIBSSH2_CHANNEL) -> c_int;
    pub fn libssh2_channel_open_ex(
        sess: *mut LIBSSH2_SESSION,
        channel_type: *const c_char,
        channel_type_len: c_uint,
        window_size: c_uint,
        packet_size: c_uint,
        message: *const c_char,
        message_len: c_uint,
    ) -> *mut LIBSSH2_CHANNEL;
    pub fn libssh2_channel_read_ex(
        chan: *mut LIBSSH2_CHANNEL,
        stream_id: c_int,
        buf: *mut c_char,
        buflen: size_t,
    ) -> ssize_t;
    pub fn libssh2_channel_setenv_ex(
        chan: *mut LIBSSH2_CHANNEL,
        var: *const c_char,
        varlen: c_uint,
        val: *const c_char,
        vallen: c_uint,
    ) -> c_int;
    pub fn libssh2_channel_send_eof(chan: *mut LIBSSH2_CHANNEL) -> c_int;
    pub fn libssh2_channel_request_pty_ex(
        chan: *mut LIBSSH2_CHANNEL,
        term: *const c_char,
        termlen: c_uint,
        modes: *const c_char,
        modeslen: c_uint,
        width: c_int,
        height: c_int,
        width_px: c_int,
        height_px: c_int,
    ) -> c_int;
    pub fn libssh2_channel_request_pty_size_ex(
        chan: *mut LIBSSH2_CHANNEL,
        width: c_int,
        height: c_int,
        width_px: c_int,
        height_px: c_int,
    ) -> c_int;
    pub fn libssh2_channel_window_read_ex(
        chan: *mut LIBSSH2_CHANNEL,
        read_avail: *mut c_ulong,
        window_size_initial: *mut c_ulong,
    ) -> c_ulong;
    pub fn libssh2_channel_window_write_ex(
        chan: *mut LIBSSH2_CHANNEL,
        window_size_initial: *mut c_ulong,
    ) -> c_ulong;
    pub fn libssh2_channel_receive_window_adjust2(
        chan: *mut LIBSSH2_CHANNEL,
        adjust: c_ulong,
        force: c_uchar,
        window: *mut c_uint,
    ) -> c_int;
    pub fn libssh2_channel_direct_tcpip_ex(
        ses: *mut LIBSSH2_SESSION,
        host: *const c_char,
        port: c_int,
        shost: *const c_char,
        sport: c_int,
    ) -> *mut LIBSSH2_CHANNEL;
    pub fn libssh2_channel_direct_streamlocal_ex(
        ses: *mut LIBSSH2_SESSION,
        socket_path: *const c_char,
        shost: *const c_char,
        sport: c_int,
    ) -> *mut LIBSSH2_CHANNEL;
    pub fn libssh2_channel_forward_accept(listener: *mut LIBSSH2_LISTENER) -> *mut LIBSSH2_CHANNEL;
    pub fn libssh2_channel_forward_cancel(listener: *mut LIBSSH2_LISTENER) -> c_int;
    pub fn libssh2_channel_forward_listen_ex(
        sess: *mut LIBSSH2_SESSION,
        host: *const c_char,
        port: c_int,
        bound_port: *mut c_int,
        queue_maxsize: c_int,
    ) -> *mut LIBSSH2_LISTENER;
    pub fn libssh2_channel_handle_extended_data2(
        channel: *mut LIBSSH2_CHANNEL,
        mode: c_int,
    ) -> c_int;
    pub fn libssh2_channel_request_auth_agent(channel: *mut LIBSSH2_CHANNEL) -> c_int;

    // userauth
    pub fn libssh2_userauth_banner(sess: *mut LIBSSH2_SESSION, banner: *mut *mut c_char) -> c_int;
    pub fn libssh2_userauth_authenticated(sess: *mut LIBSSH2_SESSION) -> c_int;
    pub fn libssh2_userauth_list(
        sess: *mut LIBSSH2_SESSION,
        username: *const c_char,
        username_len: c_uint,
    ) -> *mut c_char;
    pub fn libssh2_userauth_hostbased_fromfile_ex(
        sess: *mut LIBSSH2_SESSION,
        username: *const c_char,
        username_len: c_uint,
        publickey: *const c_char,
        privatekey: *const c_char,
        passphrase: *const c_char,
        hostname: *const c_char,
        hostname_len: c_uint,
        local_username: *const c_char,
        local_len: c_uint,
    ) -> c_int;
    pub fn libssh2_userauth_publickey_fromfile_ex(
        sess: *mut LIBSSH2_SESSION,
        username: *const c_char,
        username_len: c_uint,
        publickey: *const c_char,
        privatekey: *const c_char,
        passphrase: *const c_char,
    ) -> c_int;
    pub fn libssh2_userauth_publickey_frommemory(
        sess: *mut LIBSSH2_SESSION,
        username: *const c_char,
        username_len: size_t,
        publickeydata: *const c_char,
        publickeydata_len: size_t,
        privatekeydata: *const c_char,
        privatekeydata_len: size_t,
        passphrase: *const c_char,
    ) -> c_int;
    pub fn libssh2_userauth_password_ex(
        session: *mut LIBSSH2_SESSION,
        username: *const c_char,
        username_len: c_uint,
        password: *const c_char,
        password_len: c_uint,
        password_change_cb: Option<LIBSSH2_PASSWD_CHANGEREQ_FUNC>,
    ) -> c_int;
    pub fn libssh2_userauth_keyboard_interactive_ex(
        session: *mut LIBSSH2_SESSION,
        username: *const c_char,
        username_len: c_uint,
        callback: Option<LIBSSH2_USERAUTH_KBDINT_RESPONSE_FUNC>,
    ) -> c_int;

    // knownhost
    pub fn libssh2_knownhost_free(hosts: *mut LIBSSH2_KNOWNHOSTS);
    pub fn libssh2_knownhost_addc(
        hosts: *mut LIBSSH2_KNOWNHOSTS,
        host: *const c_char,
        salt: *const c_char,
        key: *const c_char,
        keylen: size_t,
        comment: *const c_char,
        commentlen: size_t,
        typemask: c_int,
        store: *mut *mut libssh2_knownhost,
    ) -> c_int;
    pub fn libssh2_knownhost_check(
        hosts: *mut LIBSSH2_KNOWNHOSTS,
        host: *const c_char,
        key: *const c_char,
        keylen: size_t,
        typemask: c_int,
        knownhost: *mut *mut libssh2_knownhost,
    ) -> c_int;
    pub fn libssh2_knownhost_checkp(
        hosts: *mut LIBSSH2_KNOWNHOSTS,
        host: *const c_char,
        port: c_int,
        key: *const c_char,
        keylen: size_t,
        typemask: c_int,
        knownhost: *mut *mut libssh2_knownhost,
    ) -> c_int;
    pub fn libssh2_knownhost_del(
        hosts: *mut LIBSSH2_KNOWNHOSTS,
        entry: *mut libssh2_knownhost,
    ) -> c_int;
    pub fn libssh2_knownhost_get(
        hosts: *mut LIBSSH2_KNOWNHOSTS,
        store: *mut *mut libssh2_knownhost,
        prev: *mut libssh2_knownhost,
    ) -> c_int;
    pub fn libssh2_knownhost_readfile(
        hosts: *mut LIBSSH2_KNOWNHOSTS,
        filename: *const c_char,
        kind: c_int,
    ) -> c_int;
    pub fn libssh2_knownhost_readline(
        hosts: *mut LIBSSH2_KNOWNHOSTS,
        line: *const c_char,
        len: size_t,
        kind: c_int,
    ) -> c_int;
    pub fn libssh2_knownhost_writefile(
        hosts: *mut LIBSSH2_KNOWNHOSTS,
        filename: *const c_char,
        kind: c_int,
    ) -> c_int;
    pub fn libssh2_knownhost_writeline(
        hosts: *mut LIBSSH2_KNOWNHOSTS,
        known: *mut libssh2_knownhost,
        buffer: *mut c_char,
        buflen: size_t,
        outlen: *mut size_t,
        kind: c_int,
    ) -> c_int;
    pub fn libssh2_knownhost_init(sess: *mut LIBSSH2_SESSION) -> *mut LIBSSH2_KNOWNHOSTS;

    // scp
    #[deprecated(note = "dangerously unsafe on windows, use libssh2_scp_recv2 instead")]
    pub fn libssh2_scp_recv(
        sess: *mut LIBSSH2_SESSION,
        path: *const c_char,
        sb: *mut libc::stat,
    ) -> *mut LIBSSH2_CHANNEL;

    pub fn libssh2_scp_recv2(
        sess: *mut LIBSSH2_SESSION,
        path: *const c_char,
        sb: *mut libssh2_struct_stat,
    ) -> *mut LIBSSH2_CHANNEL;

    pub fn libssh2_scp_send64(
        sess: *mut LIBSSH2_SESSION,
        path: *const c_char,
        mode: c_int,
        size: libssh2_int64_t,
        mtime: libc::time_t,
        atime: libc::time_t,
    ) -> *mut LIBSSH2_CHANNEL;

    // sftp
    pub fn libssh2_sftp_init(sess: *mut LIBSSH2_SESSION) -> *mut LIBSSH2_SFTP;
    pub fn libssh2_sftp_shutdown(sftp: *mut LIBSSH2_SFTP) -> c_int;
    pub fn libssh2_sftp_last_error(sftp: *mut LIBSSH2_SFTP) -> c_ulong;
    pub fn libssh2_sftp_open_ex(
        sftp: *mut LIBSSH2_SFTP,
        filename: *const c_char,
        filename_len: c_uint,
        flags: c_ulong,
        mode: c_long,
        open_type: c_int,
    ) -> *mut LIBSSH2_SFTP_HANDLE;
    pub fn libssh2_sftp_close_handle(handle: *mut LIBSSH2_SFTP_HANDLE) -> c_int;
    pub fn libssh2_sftp_mkdir_ex(
        sftp: *mut LIBSSH2_SFTP,
        path: *const c_char,
        path_len: c_uint,
        mode: c_long,
    ) -> c_int;
    pub fn libssh2_sftp_fsync(handle: *mut LIBSSH2_SFTP_HANDLE) -> c_int;
    pub fn libssh2_sftp_fstat_ex(
        handle: *mut LIBSSH2_SFTP_HANDLE,
        attrs: *mut LIBSSH2_SFTP_ATTRIBUTES,
        setstat: c_int,
    ) -> c_int;
    pub fn libssh2_sftp_fstatvfs(
        handle: *mut LIBSSH2_SFTP_HANDLE,
        attrs: *mut LIBSSH2_SFTP_STATVFS,
    ) -> c_int;
    pub fn libssh2_sftp_stat_ex(
        sftp: *mut LIBSSH2_SFTP,
        path: *const c_char,
        path_len: c_uint,
        stat_type: c_int,
        attrs: *mut LIBSSH2_SFTP_ATTRIBUTES,
    ) -> c_int;
    pub fn libssh2_sftp_read(
        handle: *mut LIBSSH2_SFTP_HANDLE,
        buf: *mut c_char,
        len: size_t,
    ) -> ssize_t;
    pub fn libssh2_sftp_symlink_ex(
        sftp: *mut LIBSSH2_SFTP,
        path: *const c_char,
        path_len: c_uint,
        target: *mut c_char,
        target_len: c_uint,
        link_type: c_int,
    ) -> c_int;
    pub fn libssh2_sftp_rename_ex(
        sftp: *mut LIBSSH2_SFTP,
        src: *const c_char,
        src_len: c_uint,
        dst: *const c_char,
        dst_len: c_uint,
        flags: c_long,
    ) -> c_int;
    pub fn libssh2_sftp_rmdir_ex(
        sftp: *mut LIBSSH2_SFTP,
        path: *const c_char,
        path_len: c_uint,
    ) -> c_int;
    pub fn libssh2_sftp_write(
        handle: *mut LIBSSH2_SFTP_HANDLE,
        buffer: *const c_char,
        len: size_t,
    ) -> ssize_t;
    pub fn libssh2_sftp_tell64(handle: *mut LIBSSH2_SFTP_HANDLE) -> libssh2_uint64_t;
    pub fn libssh2_sftp_seek64(handle: *mut LIBSSH2_SFTP_HANDLE, off: libssh2_uint64_t);
    pub fn libssh2_sftp_readdir_ex(
        handle: *mut LIBSSH2_SFTP_HANDLE,
        buffer: *mut c_char,
        buffer_len: size_t,
        longentry: *mut c_char,
        longentry_len: size_t,
        attrs: *mut LIBSSH2_SFTP_ATTRIBUTES,
    ) -> c_int;
    pub fn libssh2_sftp_unlink_ex(
        sftp: *mut LIBSSH2_SFTP,
        filename: *const c_char,
        filename_len: c_uint,
    ) -> c_int;
}

#[test]
fn smoke() {
    unsafe { libssh2_init(0) };
}

#[doc(hidden)]
pub fn issue_14344_workaround() {}

pub fn init() {
    use std::sync::Once;

    static INIT: Once = Once::new();
    INIT.call_once(|| unsafe {
        platform_init();
        assert_eq!(libc::atexit(shutdown), 0);
    });
    extern "C" fn shutdown() {
        unsafe {
            libssh2_exit();
        }
    }

    #[cfg(unix)]
    unsafe fn platform_init() {
        // On Unix we want to funnel through openssl_sys to initialize OpenSSL,
        // so be sure to tell libssh2 to not do its own thing as we've already
        // taken care of it.
        openssl_sys::init();
        assert_eq!(libssh2_init(LIBSSH2_INIT_NO_CRYPTO), 0);
    }

    #[cfg(windows)]
    unsafe fn platform_init() {
        // On Windows we want to be sure to tell libssh2 to initialize
        // everything, as we're not managing crypto elsewhere ourselves. Also to
        // fix alexcrichton/git2-rs#202
        assert_eq!(libssh2_init(0), 0);
    }
}
