use std::collections::HashMap;

pub fn parse_cookie_header(cookie_header: &str) -> HashMap<String, String> {
    let mut cookie_map = HashMap::new();

    // Split the cookie string into individual cookies
    for cookie in cookie_header.split(';') {
        let trimmed_cookie = cookie.trim(); // Remove leading/trailing spaces

        // Split each cookie into key=value
        if let Some(eq_pos) = trimmed_cookie.find('=') {
            let key = &trimmed_cookie[..eq_pos];
            let value = &trimmed_cookie[eq_pos + 1..];

            // Insert the key-value pair into the HashMap
            cookie_map.insert(key.to_string(), value.to_string());
        }
    }

    cookie_map
}
