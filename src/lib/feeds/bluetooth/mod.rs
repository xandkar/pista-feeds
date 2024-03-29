use std::{fs, io::BufRead, time::Duration};

use anyhow::{anyhow, Result};

struct State<'a> {
    prefix: &'a str,
    device_state: Option<DeviceState>,
}

impl<'a> State<'a> {
    fn new(prefix: &'a str) -> Self {
        Self {
            prefix,
            device_state: None,
        }
    }
}

impl<'a> crate::pipeline::State for State<'a> {
    type Event = Option<DeviceState>;

    fn update(
        &mut self,
        dev_opt: Self::Event,
    ) -> Result<Option<Vec<crate::alert::Alert>>> {
        self.device_state = dev_opt;
        Ok(None)
    }

    fn display<W: std::io::Write>(&self, mut buf: W) -> Result<()> {
        match self.device_state {
            Some(DeviceState::NoDev) | None => {
                writeln!(buf, "{} ", self.prefix)?
            }
            // TODO Distinguish between OffSoft and OffHard
            Some(DeviceState::OffSoft | DeviceState::OffHard) => {
                writeln!(buf, "{}-", self.prefix)?
            }
            Some(DeviceState::On { conn_count: None }) => {
                writeln!(buf, "{}+", self.prefix)?
            }
            Some(DeviceState::On {
                conn_count: Some(c),
            }) => writeln!(buf, "{}{}", self.prefix, c)?,
        };
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum ConnCount {
    No,
    Yes { timeout: Duration },
}

#[derive(Debug)]
enum DeviceState {
    NoDev,
    OffHard,
    OffSoft,
    On { conn_count: Option<usize> },
}

impl DeviceState {
    fn read(conn_count: ConnCount) -> Result<Option<Self>> {
        // This method of device state lookup is taken from TLP bluetooth command.
        // TODO Checkout https://crates.io/crates/bluer
        let mut bt_state_opt: Option<Self> = None;
        for entry in fs::read_dir("/sys/class/rfkill/")? {
            let entry = entry?;
            let mut path_type = entry.path();
            let mut path_state = entry.path();
            path_type.push("type");
            path_state.push("state");
            if let "bluetooth" = fs::read_to_string(path_type)?.trim_end() {
                let state_byte: u8 =
                    fs::read_to_string(path_state)?.trim_end().parse()?;
                bt_state_opt = Some(Self::from_byte(state_byte, conn_count)?);
                return Ok(bt_state_opt);
            }
        }
        Ok(bt_state_opt)
    }

    fn from_byte(b: u8, conn_count: ConnCount) -> Result<Self> {
        let selph = match b {
            0 => Self::OffSoft,
            1 => {
                let conn_count = match conn_count {
                    ConnCount::No => None,
                    ConnCount::Yes { timeout } => Self::conn_count(timeout),
                };
                Self::On { conn_count }
            }
            2 => Self::OffHard,
            254 => Self::NoDev,
            _ => return Err(anyhow!("Invalid state byte: {:?}", b)),
        };
        Ok(selph)
    }

    fn conn_count(timeout: Duration) -> Option<usize> {
        crate::process::exec_with_timeout(
            "bluetoothctl",
            &["--", "devices", "Connected"],
            timeout,
        )
        .and_then(|result| result.ok().map(|output| output.lines().count()))
    }
}

pub fn run(
    prefix: &str,
    interval: Duration,
    conn_count_enabled: bool,
    conn_count_timeout: Duration,
) -> Result<()> {
    use crate::clock;

    let conn_count = if conn_count_enabled {
        ConnCount::Yes {
            timeout: conn_count_timeout,
        }
    } else {
        ConnCount::No
    };

    let events = clock::new(interval)
        .map(|clock::Tick| DeviceState::read(conn_count))
        .filter_map(|result| match result {
            Err(error) => {
                tracing::error!(?error, "Failed to read device state.");
                None
            }
            Ok(dev_opt) => Some(dev_opt),
        });
    crate::pipeline::run_to_stdout(events, State::new(prefix))
}
