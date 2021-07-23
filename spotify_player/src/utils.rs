pub fn format_duration(duration: u32) -> String {
    format!("{}:{:02}", duration / 60000, (duration / 1000) % 60)
}
