use serde_json::Value;

pub(crate) fn next_turn_index(
    last_run: Option<&Value>,
    operation_id: Option<&str>,
    observed_ops: &[String],
) -> u64 {
    let observed_max = observed_ops.iter().filter_map(|op| parse_turn_index(op)).max();
    last_run
        .and_then(|value| value.get("turn_index"))
        .and_then(Value::as_u64)
        .or_else(|| operation_id.and_then(parse_turn_index))
        .or(observed_max)
        .unwrap_or(0)
        + 1
}

pub(crate) fn parse_turn_index(operation_id: &str) -> Option<u64> {
    operation_id
        .rsplit_once('-')
        .and_then(|(_, tail)| tail.parse::<u64>().ok())
}

#[cfg(test)]
mod tests {
    use super::{next_turn_index, parse_turn_index};
    use serde_json::json;

    #[test]
    fn parse_turn_index_reads_suffix_digits() {
        assert_eq!(parse_turn_index("op-demo-0009"), Some(9));
        assert_eq!(parse_turn_index("op-demo"), None);
    }

    #[test]
    fn next_turn_index_prefers_last_run_counter() {
        let last_run = json!({"turn_index": 9_u64});
        assert_eq!(next_turn_index(Some(&last_run), Some("op-demo-0008"), &[]), 10);
    }

    #[test]
    fn next_turn_index_falls_back_to_observed_history() {
        let observed = vec!["op-demo-0001".into(), "op-demo-0009".into()];
        assert_eq!(next_turn_index(None, None, &observed), 10);
    }
}
