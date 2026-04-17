use chrono::{DateTime, Duration, Local};

pub(crate) fn local_timestamp_now() -> String {
    format_local_timestamp(Local::now())
}

pub(crate) fn local_timestamp_for_turn(base: DateTime<Local>, turn_index: usize) -> String {
    format_local_timestamp(base + Duration::seconds(turn_index as i64))
}

pub(crate) fn local_time_base() -> DateTime<Local> {
    Local::now()
}

fn format_local_timestamp(value: DateTime<Local>) -> String {
    value.format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}
