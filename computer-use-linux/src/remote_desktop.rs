use crate::diagnostics::hydrate_session_bus_env;
use anyhow::{bail, Context, Result};
use futures_util::StreamExt;
use std::{collections::HashMap, time::Duration};
use zbus::{
    proxy::SignalStream,
    zvariant::{OwnedObjectPath, OwnedValue, Value},
    Connection, Proxy,
};

const PORTAL_DESKTOP_SERVICE: &str = "org.freedesktop.portal.Desktop";
const PORTAL_DESKTOP_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_REMOTE_DESKTOP_INTERFACE: &str = "org.freedesktop.portal.RemoteDesktop";
const PORTAL_SCREENCAST_INTERFACE: &str = "org.freedesktop.portal.ScreenCast";
const PORTAL_REQUEST_INTERFACE: &str = "org.freedesktop.portal.Request";
const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

const DEVICE_POINTER: u32 = 2;
const SOURCE_MONITOR: u32 = 1;
const CURSOR_MODE_HIDDEN: u32 = 1;

const POINTER_BUTTON_RELEASED: u32 = 0;
const POINTER_BUTTON_PRESSED: u32 = 1;

const AXIS_VERTICAL: u32 = 0;
const AXIS_HORIZONTAL: u32 = 1;

const BTN_LEFT: i32 = 0x110;
const BTN_RIGHT: i32 = 0x111;
const BTN_MIDDLE: i32 = 0x112;
const BTN_SIDE: i32 = 0x113;
const BTN_EXTRA: i32 = 0x114;
const BTN_FORWARD: i32 = 0x115;
const BTN_BACK: i32 = 0x116;

#[derive(Clone)]
pub struct PortalPointerSession {
    connection: Connection,
    session_handle: OwnedObjectPath,
    streams: Vec<PortalStream>,
}

#[derive(Debug, Clone)]
struct PortalStream {
    node_id: u32,
    position: Option<(i32, i32)>,
    size: Option<(i32, i32)>,
}

#[derive(Debug, Clone, Copy)]
pub enum PointerButton {
    Left,
    Right,
    Middle,
    Side,
    Extra,
    Forward,
    Back,
}

#[derive(Debug, Clone, Copy)]
pub enum ScrollDirection {
    Up,
    Down,
    Left,
    Right,
}

pub async fn start_portal_pointer_session() -> Result<PortalPointerSession> {
    hydrate_session_bus_env();

    let connection = Connection::session()
        .await
        .context("failed to connect to session bus for remote desktop portal")?;
    let session_handle = create_remote_desktop_session(&connection).await?;
    select_pointer_devices(&connection, &session_handle).await?;
    select_monitor_sources(&connection, &session_handle).await?;
    let (devices, streams) = start_remote_desktop_session(&connection, &session_handle).await?;

    if devices & DEVICE_POINTER == 0 {
        bail!("remote desktop portal session started without pointer access");
    }
    if streams.is_empty() {
        bail!("remote desktop portal session started without any monitor streams");
    }

    Ok(PortalPointerSession {
        connection,
        session_handle,
        streams,
    })
}

pub async fn click(
    session: &PortalPointerSession,
    x: i32,
    y: i32,
    button: PointerButton,
    click_count: u32,
) -> Result<()> {
    let proxy = remote_desktop_proxy(&session.connection).await?;
    let (stream_id, x, y) = session.map_absolute_point(x, y)?;
    notify_pointer_motion_absolute(&proxy, &session.session_handle, stream_id, x, y).await?;
    for _ in 0..click_count.max(1) {
        notify_pointer_button(
            &proxy,
            &session.session_handle,
            button.evdev_code(),
            POINTER_BUTTON_PRESSED,
        )
        .await?;
        tokio::time::sleep(Duration::from_millis(35)).await;
        notify_pointer_button(
            &proxy,
            &session.session_handle,
            button.evdev_code(),
            POINTER_BUTTON_RELEASED,
        )
        .await?;
    }
    Ok(())
}

pub async fn scroll(
    session: &PortalPointerSession,
    target_point: Option<(i32, i32)>,
    direction: ScrollDirection,
    steps: i32,
) -> Result<()> {
    let proxy = remote_desktop_proxy(&session.connection).await?;
    if let Some((x, y)) = target_point {
        let (stream_id, x, y) = session.map_absolute_point(x, y)?;
        notify_pointer_motion_absolute(&proxy, &session.session_handle, stream_id, x, y).await?;
    }

    let (axis, steps) = match direction {
        ScrollDirection::Up => (AXIS_VERTICAL, steps.max(1)),
        ScrollDirection::Down => (AXIS_VERTICAL, -steps.max(1)),
        ScrollDirection::Left => (AXIS_HORIZONTAL, steps.max(1)),
        ScrollDirection::Right => (AXIS_HORIZONTAL, -steps.max(1)),
    };

    notify_pointer_axis_discrete(&proxy, &session.session_handle, axis, steps).await
}

pub async fn drag(
    session: &PortalPointerSession,
    start_x: i32,
    start_y: i32,
    end_x: i32,
    end_y: i32,
) -> Result<()> {
    let proxy = remote_desktop_proxy(&session.connection).await?;
    let (start_stream, start_x, start_y) = session.map_absolute_point(start_x, start_y)?;
    notify_pointer_motion_absolute(
        &proxy,
        &session.session_handle,
        start_stream,
        start_x,
        start_y,
    )
    .await?;
    notify_pointer_button(
        &proxy,
        &session.session_handle,
        BTN_LEFT,
        POINTER_BUTTON_PRESSED,
    )
    .await?;
    tokio::time::sleep(Duration::from_millis(35)).await;
    let (end_stream, end_x, end_y) = session.map_absolute_point(end_x, end_y)?;
    notify_pointer_motion_absolute(&proxy, &session.session_handle, end_stream, end_x, end_y)
        .await?;
    tokio::time::sleep(Duration::from_millis(35)).await;
    notify_pointer_button(
        &proxy,
        &session.session_handle,
        BTN_LEFT,
        POINTER_BUTTON_RELEASED,
    )
    .await
}

impl PortalPointerSession {
    fn map_absolute_point(&self, x: i32, y: i32) -> Result<(u32, f64, f64)> {
        if let Some(stream) = self
            .streams
            .iter()
            .find(|stream| stream.contains_global_point(x, y))
        {
            return Ok(stream.relative_point(x, y));
        }

        self.streams
            .first()
            .map(|stream| stream.relative_point(x, y))
            .context("remote desktop portal session had no usable streams")
    }
}

impl PortalStream {
    fn contains_global_point(&self, x: i32, y: i32) -> bool {
        let Some((stream_x, stream_y)) = self.position else {
            return false;
        };
        let Some((width, height)) = self.size else {
            return false;
        };
        x >= stream_x && y >= stream_y && x < stream_x + width && y < stream_y + height
    }

    fn relative_point(&self, x: i32, y: i32) -> (u32, f64, f64) {
        let (stream_x, stream_y) = self.position.unwrap_or((0, 0));
        let (width, height) = self.size.unwrap_or((i32::MAX, i32::MAX));
        let rel_x = (x - stream_x).clamp(0, width.saturating_sub(1)) as f64;
        let rel_y = (y - stream_y).clamp(0, height.saturating_sub(1)) as f64;
        (self.node_id, rel_x, rel_y)
    }
}

impl PointerButton {
    pub fn from_name(name: Option<&str>) -> Self {
        match name.unwrap_or("left").to_ascii_lowercase().as_str() {
            "right" => Self::Right,
            "middle" => Self::Middle,
            "side" => Self::Side,
            "extra" => Self::Extra,
            "forward" => Self::Forward,
            "back" => Self::Back,
            _ => Self::Left,
        }
    }

    fn evdev_code(self) -> i32 {
        match self {
            Self::Left => BTN_LEFT,
            Self::Right => BTN_RIGHT,
            Self::Middle => BTN_MIDDLE,
            Self::Side => BTN_SIDE,
            Self::Extra => BTN_EXTRA,
            Self::Forward => BTN_FORWARD,
            Self::Back => BTN_BACK,
        }
    }
}

async fn create_remote_desktop_session(connection: &Connection) -> Result<OwnedObjectPath> {
    let remote_proxy = remote_desktop_proxy(connection).await?;
    let (request_path, mut response_stream) =
        portal_request_stream(connection, "rd_create").await?;
    let session_token = request_token("rd_session");
    let mut options: HashMap<&str, Value<'_>> = HashMap::new();
    options.insert(
        "handle_token",
        Value::from(last_path_component(&request_path)),
    );
    options.insert("session_handle_token", Value::from(session_token.as_str()));

    let handle: OwnedObjectPath = remote_proxy
        .call("CreateSession", &(options))
        .await
        .context("RemoteDesktop CreateSession call failed")?;
    let (response_code, results) =
        await_portal_response(connection, handle, &request_path, &mut response_stream).await?;
    if response_code != 0 {
        bail!("RemoteDesktop CreateSession denied or cancelled with response code {response_code}");
    }

    let session_handle: String = results
        .get("session_handle")
        .context("RemoteDesktop CreateSession response did not include session_handle")?
        .try_clone()
        .context("failed to clone session_handle")?
        .try_into()
        .context("RemoteDesktop session_handle was not a string")?;
    OwnedObjectPath::try_from(session_handle)
        .context("RemoteDesktop session_handle was not a valid object path")
}

async fn select_pointer_devices(connection: &Connection, session: &OwnedObjectPath) -> Result<()> {
    let remote_proxy = remote_desktop_proxy(connection).await?;
    let (request_path, mut response_stream) =
        portal_request_stream(connection, "rd_devices").await?;
    let mut options: HashMap<&str, Value<'_>> = HashMap::new();
    options.insert(
        "handle_token",
        Value::from(last_path_component(&request_path)),
    );
    options.insert("types", Value::from(DEVICE_POINTER));

    let handle: OwnedObjectPath = remote_proxy
        .call("SelectDevices", &(session, options))
        .await
        .context("RemoteDesktop SelectDevices call failed")?;
    let (response_code, _) =
        await_portal_response(connection, handle, &request_path, &mut response_stream).await?;
    if response_code != 0 {
        bail!("RemoteDesktop SelectDevices denied or cancelled with response code {response_code}");
    }
    Ok(())
}

async fn select_monitor_sources(connection: &Connection, session: &OwnedObjectPath) -> Result<()> {
    let screencast_proxy = screencast_proxy(connection).await?;
    let (request_path, mut response_stream) =
        portal_request_stream(connection, "rd_sources").await?;
    let mut options: HashMap<&str, Value<'_>> = HashMap::new();
    options.insert(
        "handle_token",
        Value::from(last_path_component(&request_path)),
    );
    options.insert("types", Value::from(SOURCE_MONITOR));
    options.insert("multiple", Value::from(false));
    options.insert("cursor_mode", Value::from(CURSOR_MODE_HIDDEN));

    let handle: OwnedObjectPath = screencast_proxy
        .call("SelectSources", &(session, options))
        .await
        .context("ScreenCast SelectSources call failed for remote desktop session")?;
    let (response_code, _) =
        await_portal_response(connection, handle, &request_path, &mut response_stream).await?;
    if response_code != 0 {
        bail!("ScreenCast SelectSources denied or cancelled with response code {response_code}");
    }
    Ok(())
}

async fn start_remote_desktop_session(
    connection: &Connection,
    session: &OwnedObjectPath,
) -> Result<(u32, Vec<PortalStream>)> {
    let remote_proxy = remote_desktop_proxy(connection).await?;
    let (request_path, mut response_stream) = portal_request_stream(connection, "rd_start").await?;
    let mut options: HashMap<&str, Value<'_>> = HashMap::new();
    options.insert(
        "handle_token",
        Value::from(last_path_component(&request_path)),
    );

    let handle: OwnedObjectPath = remote_proxy
        .call("Start", &(session, "", options))
        .await
        .context("RemoteDesktop Start call failed")?;
    let (response_code, results) =
        await_portal_response(connection, handle, &request_path, &mut response_stream).await?;
    if response_code != 0 {
        bail!("RemoteDesktop Start denied or cancelled with response code {response_code}");
    }

    let devices = results
        .get("devices")
        .and_then(|value| u32::try_from(value).ok())
        .unwrap_or_default();
    let streams = results
        .get("streams")
        .map(parse_streams)
        .transpose()?
        .unwrap_or_default();
    Ok((devices, streams))
}

async fn notify_pointer_motion_absolute(
    proxy: &Proxy<'_>,
    session: &OwnedObjectPath,
    stream_id: u32,
    x: f64,
    y: f64,
) -> Result<()> {
    let options: HashMap<&str, Value<'_>> = HashMap::new();
    let _: () = proxy
        .call(
            "NotifyPointerMotionAbsolute",
            &(session, options, stream_id, x, y),
        )
        .await
        .context("RemoteDesktop NotifyPointerMotionAbsolute failed")?;
    Ok(())
}

async fn notify_pointer_button(
    proxy: &Proxy<'_>,
    session: &OwnedObjectPath,
    button: i32,
    state: u32,
) -> Result<()> {
    let options: HashMap<&str, Value<'_>> = HashMap::new();
    let _: () = proxy
        .call("NotifyPointerButton", &(session, options, button, state))
        .await
        .context("RemoteDesktop NotifyPointerButton failed")?;
    Ok(())
}

async fn notify_pointer_axis_discrete(
    proxy: &Proxy<'_>,
    session: &OwnedObjectPath,
    axis: u32,
    steps: i32,
) -> Result<()> {
    let options: HashMap<&str, Value<'_>> = HashMap::new();
    let _: () = proxy
        .call(
            "NotifyPointerAxisDiscrete",
            &(session, options, axis, steps),
        )
        .await
        .context("RemoteDesktop NotifyPointerAxisDiscrete failed")?;
    Ok(())
}

async fn remote_desktop_proxy(connection: &Connection) -> Result<Proxy<'_>> {
    Proxy::new(
        connection,
        PORTAL_DESKTOP_SERVICE,
        PORTAL_DESKTOP_PATH,
        PORTAL_REMOTE_DESKTOP_INTERFACE,
    )
    .await
    .context("failed to create RemoteDesktop portal proxy")
}

async fn screencast_proxy(connection: &Connection) -> Result<Proxy<'_>> {
    Proxy::new(
        connection,
        PORTAL_DESKTOP_SERVICE,
        PORTAL_DESKTOP_PATH,
        PORTAL_SCREENCAST_INTERFACE,
    )
    .await
    .context("failed to create ScreenCast portal proxy")
}

async fn portal_request_stream<'a>(
    connection: &'a Connection,
    prefix: &str,
) -> Result<(String, SignalStream<'a>)> {
    let unique_name = connection
        .unique_name()
        .context("session bus connection has no unique name")?;
    let token = request_token(prefix);
    let request_path = request_path(unique_name.as_str(), &token);
    let request_proxy = Proxy::new(
        connection,
        PORTAL_DESKTOP_SERVICE,
        request_path.as_str(),
        PORTAL_REQUEST_INTERFACE,
    )
    .await
    .context("failed to create portal request proxy")?;
    let response_stream = request_proxy
        .receive_signal("Response")
        .await
        .context("failed to subscribe to portal request response")?;
    Ok((request_path, response_stream))
}

async fn await_portal_response(
    connection: &Connection,
    handle: OwnedObjectPath,
    expected_request_path: &str,
    response_stream: &mut SignalStream<'_>,
) -> Result<(u32, HashMap<String, OwnedValue>)> {
    if handle.as_str() != expected_request_path {
        *response_stream = Proxy::new(
            connection,
            PORTAL_DESKTOP_SERVICE,
            handle.as_str(),
            PORTAL_REQUEST_INTERFACE,
        )
        .await
        .context("failed to create returned portal request proxy")?
        .receive_signal("Response")
        .await
        .context("failed to subscribe to returned portal response")?;
    }

    let response = tokio::time::timeout(REQUEST_TIMEOUT, response_stream.next())
        .await
        .context("timed out waiting for portal response")?
        .context("portal response stream ended")?;
    response
        .body()
        .deserialize()
        .context("failed to decode portal response")
}

fn parse_streams(value: &OwnedValue) -> Result<Vec<PortalStream>> {
    let streams: Vec<(u32, HashMap<String, OwnedValue>)> = value
        .try_clone()
        .context("failed to clone streams response")?
        .try_into()
        .context("portal streams response had unexpected type")?;
    Ok(streams
        .into_iter()
        .map(|(node_id, properties)| PortalStream {
            node_id,
            position: get_pair_i32(&properties, "position"),
            size: get_pair_i32(&properties, "size"),
        })
        .collect())
}

fn get_pair_i32(properties: &HashMap<String, OwnedValue>, key: &str) -> Option<(i32, i32)> {
    properties.get(key).and_then(|value| {
        value
            .try_clone()
            .ok()
            .and_then(|owned| <(i32, i32)>::try_from(owned).ok())
            .or_else(|| {
                value
                    .try_clone()
                    .ok()
                    .and_then(|owned| <(u32, u32)>::try_from(owned).ok())
                    .map(|(left, right)| (left as i32, right as i32))
            })
    })
}

fn request_path(unique_name: &str, token: &str) -> String {
    format!(
        "/org/freedesktop/portal/desktop/request/{}/{}",
        unique_name.trim_start_matches(':').replace('.', "_"),
        token
    )
}

fn last_path_component(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn request_token(prefix: &str) -> String {
    format!(
        "{prefix}_{}_{:?}",
        std::process::id(),
        std::time::SystemTime::now()
    )
    .chars()
    .map(|ch| match ch {
        'a'..='z' | 'A'..='Z' | '0'..='9' | '_' => ch,
        _ => '_',
    })
    .collect()
}
