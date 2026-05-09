use std::fmt;

#[derive(Debug)]
pub struct Error {
    code: u32,
    extra: Option<String>,
}

impl Error {
    pub fn new(code: u32) -> Error {
        Error { code, extra: None }
    }

    pub fn set_extra(&mut self, extra: String) {
        self.extra = Some(extra);
    }

    pub fn is_aborted_by_callback(&self) -> bool {
        false
    }

    pub fn is_couldnt_connect(&self) -> bool {
        false
    }

    pub fn is_couldnt_resolve_proxy(&self) -> bool {
        false
    }

    pub fn is_couldnt_resolve_host(&self) -> bool {
        false
    }

    pub fn is_operation_timedout(&self) -> bool {
        false
    }

    pub fn is_recv_error(&self) -> bool {
        false
    }

    pub fn is_send_error(&self) -> bool {
        false
    }

    pub fn is_http2_error(&self) -> bool {
        false
    }

    pub fn is_http2_stream_error(&self) -> bool {
        false
    }

    pub fn is_ssl_connect_error(&self) -> bool {
        false
    }

    pub fn is_partial_file(&self) -> bool {
        false
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.extra {
            Some(extra) => write!(f, "WASI Cargo curl shim error {}: {}", self.code, extra),
            None => write!(f, "WASI Cargo curl shim error {}", self.code),
        }
    }
}

impl std::error::Error for Error {}

#[derive(Debug)]
pub struct Version;

impl Version {
    pub fn get() -> Version {
        Version
    }

    pub fn vendored(&self) -> bool {
        false
    }

    pub fn version(&self) -> &'static str {
        "wasi-shim"
    }

    pub fn ssl_version(&self) -> Option<&'static str> {
        None
    }
}

pub mod easy {
    use super::Error;
    use std::path::Path;
    use std::time::Duration;

    #[derive(Debug, Default)]
    pub struct Easy {
        url: Option<String>,
        response_code: u32,
    }

    #[derive(Debug, Default)]
    pub struct List;

    impl List {
        pub fn new() -> List {
            List
        }

        pub fn append(&mut self, _header: &str) -> Result<(), Error> {
            Ok(())
        }
    }

    #[derive(Clone, Copy, Debug)]
    pub enum HttpVersion {
        V11,
        V2,
    }

    #[derive(Clone, Copy, Debug)]
    pub enum InfoType {
        Text,
        HeaderIn,
        HeaderOut,
        DataIn,
        DataOut,
        SslDataIn,
        SslDataOut,
        End,
    }

    #[derive(Clone, Copy, Debug)]
    pub enum SslVersion {
        Default,
        Tlsv1,
        Tlsv10,
        Tlsv11,
        Tlsv12,
        Tlsv13,
    }

    #[derive(Clone, Copy, Debug, Default)]
    pub struct SslOpt {
        no_revoke: bool,
    }

    impl SslOpt {
        pub fn new() -> SslOpt {
            SslOpt { no_revoke: false }
        }

        pub fn no_revoke(mut self, value: bool) -> SslOpt {
            self.no_revoke = value;
            self
        }
    }

    impl Easy {
        pub fn new() -> Easy {
            Easy::default()
        }

        pub fn reset(&mut self) {
            self.url = None;
            self.response_code = 0;
        }

        pub fn proxy(&mut self, _proxy: &str) -> Result<(), Error> {
            Ok(())
        }

        pub fn cainfo<P: AsRef<Path>>(&mut self, _path: P) -> Result<(), Error> {
            Ok(())
        }

        pub fn proxy_cainfo(&mut self, _path: &str) -> Result<(), Error> {
            Ok(())
        }

        pub fn ssl_options(&mut self, _options: SslOpt) -> Result<(), Error> {
            Ok(())
        }

        pub fn useragent(&mut self, _value: &str) -> Result<(), Error> {
            Ok(())
        }

        pub fn accept_encoding(&mut self, _value: &str) -> Result<(), Error> {
            Ok(())
        }

        pub fn ssl_version(&mut self, _version: SslVersion) -> Result<(), Error> {
            Ok(())
        }

        pub fn ssl_min_max_version(
            &mut self,
            _min: SslVersion,
            _max: SslVersion,
        ) -> Result<(), Error> {
            Ok(())
        }

        pub fn verbose(&mut self, _value: bool) -> Result<(), Error> {
            Ok(())
        }

        pub fn debug_function<F>(&mut self, _f: F) -> Result<(), Error>
        where
            F: FnMut(InfoType, &[u8]),
        {
            Ok(())
        }

        pub fn connect_timeout(&mut self, _duration: Duration) -> Result<(), Error> {
            Ok(())
        }

        pub fn low_speed_time(&mut self, _duration: Duration) -> Result<(), Error> {
            Ok(())
        }

        pub fn low_speed_limit(&mut self, _limit: u32) -> Result<(), Error> {
            Ok(())
        }

        pub fn get(&mut self, _value: bool) -> Result<(), Error> {
            Ok(())
        }

        pub fn put(&mut self, _value: bool) -> Result<(), Error> {
            Ok(())
        }

        pub fn custom_request(&mut self, _request: &str) -> Result<(), Error> {
            Ok(())
        }

        pub fn upload(&mut self, _value: bool) -> Result<(), Error> {
            Ok(())
        }

        pub fn in_filesize(&mut self, _size: u64) -> Result<(), Error> {
            Ok(())
        }

        pub fn url(&mut self, url: &str) -> Result<(), Error> {
            self.url = Some(url.to_owned());
            Ok(())
        }

        pub fn follow_location(&mut self, _value: bool) -> Result<(), Error> {
            Ok(())
        }

        pub fn http_headers(&mut self, _headers: List) -> Result<(), Error> {
            Ok(())
        }

        pub fn http_version(&mut self, _version: HttpVersion) -> Result<(), Error> {
            Ok(())
        }

        pub fn pipewait(&mut self, _value: bool) -> Result<(), Error> {
            Ok(())
        }

        pub fn write_function<F>(&mut self, _f: F) -> Result<(), Error>
        where
            F: FnMut(&[u8]) -> Result<usize, std::io::Error> + 'static,
        {
            Ok(())
        }

        pub fn header_function<F>(&mut self, _f: F) -> Result<(), Error>
        where
            F: FnMut(&[u8]) -> bool + 'static,
        {
            Ok(())
        }

        pub fn progress(&mut self, _value: bool) -> Result<(), Error> {
            Ok(())
        }

        pub fn progress_function<F>(&mut self, _f: F) -> Result<(), Error>
        where
            F: FnMut(f64, f64, f64, f64) -> bool + 'static,
        {
            Ok(())
        }

        pub fn response_code(&mut self) -> Result<u32, Error> {
            Ok(self.response_code)
        }

        pub fn primary_ip(&mut self) -> Result<Option<&'static str>, Error> {
            Ok(None)
        }

        pub fn effective_url(&mut self) -> Result<Option<&str>, Error> {
            Ok(self.url.as_deref())
        }

        pub fn transfer(&mut self) -> Transfer<'_> {
            Transfer { easy: self }
        }
    }

    pub struct Transfer<'a> {
        easy: &'a mut Easy,
    }

    impl<'a> Transfer<'a> {
        pub fn read_function<F>(&mut self, _f: F) -> Result<(), Error>
        where
            F: FnMut(&mut [u8]) -> Result<usize, std::io::Error>,
        {
            Ok(())
        }

        pub fn write_function<F>(&mut self, _f: F) -> Result<(), Error>
        where
            F: FnMut(&[u8]) -> Result<usize, std::io::Error>,
        {
            Ok(())
        }

        pub fn header_function<F>(&mut self, _f: F) -> Result<(), Error>
        where
            F: FnMut(&[u8]) -> bool,
        {
            Ok(())
        }

        pub fn perform(&mut self) -> Result<(), Error> {
            self.easy.response_code = 0;
            Ok(())
        }
    }
}

pub mod multi {
    use super::Error;
    use super::easy::Easy;
    use std::time::Duration;

    #[derive(Debug, Default)]
    pub struct Multi;

    #[derive(Debug)]
    pub struct EasyHandle {
        token: usize,
        easy: Easy,
    }

    pub struct Message;

    impl Multi {
        pub fn new() -> Multi {
            Multi
        }

        pub fn pipelining(&mut self, _pipelining: bool, _multiplexing: bool) -> Result<(), Error> {
            Ok(())
        }

        pub fn set_max_host_connections(&mut self, _max: u32) -> Result<(), Error> {
            Ok(())
        }

        pub fn add(&self, easy: Easy) -> Result<EasyHandle, Error> {
            Ok(EasyHandle { token: 0, easy })
        }

        pub fn remove(&self, handle: EasyHandle) -> Result<Easy, Error> {
            Ok(handle.easy)
        }

        pub fn perform(&self) -> Result<u32, Error> {
            Ok(0)
        }

        pub fn messages<F>(&self, _f: F)
        where
            F: FnMut(Message),
        {
        }

        pub fn get_timeout(&self) -> Result<Option<Duration>, Error> {
            Ok(Some(Duration::from_secs(1)))
        }

        pub fn wait(&self, _fds: &mut [&mut ()], _timeout: Duration) -> Result<(), Error> {
            Ok(())
        }
    }

    impl EasyHandle {
        pub fn set_token(&mut self, token: usize) -> Result<(), Error> {
            self.token = token;
            Ok(())
        }

        pub fn response_code(&mut self) -> Result<u32, Error> {
            self.easy.response_code()
        }
    }

    impl Message {
        pub fn token(&self) -> Option<usize> {
            None
        }

        pub fn result_for(&self, _handle: &EasyHandle) -> Option<Result<(), Error>> {
            None
        }
    }
}
