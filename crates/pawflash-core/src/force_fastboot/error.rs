use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
pub enum Error {
    #[error("failed to open serial port `{port}`")]
    #[diagnostic(help("check port permissions (sudo or install udev rules)"))]
    OpenSerialPort {
        port: String,
        #[source]
        source: tokio_serial::Error,
    },

    #[error("serial port enumeration failed: {0}")]
    PortEnumeration(#[from] tokio_serial::Error),

    #[error("timed out waiting for preloader serial port")]
    #[diagnostic(help("ensure the device is in preloader mode (hold volume buttons while connecting USB)"))]
    PreloaderTimeout,

    #[error("preloader port `{port}` was lost before any FASTBOOT write completed")]
    #[diagnostic(help("unplug and reconnect the device, then retry force fastboot"))]
    PortLostBeforeWrite { port: String },
}

pub type Result<T> = std::result::Result<T, Error>;
