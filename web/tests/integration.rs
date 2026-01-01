use web::{Server, WebFetchInput, WebSearchInput};

fn create_server() -> Server {
    Server::new()
}

// ==================== web_fetch tests ====================

#[tokio::test]
async fn test_fetch_html_page() {
    let server = create_server();
    let input = WebFetchInput {
        url: "https://example.com".to_string(),
        timeout_ms: None,
        max_length: None,
    };

    let result = server.fetch(input).await;

    match result {
        Ok(output) => {
            println!("=== test_fetch_html_page ===");
            println!("Status: {}", output.status);
            println!("Content-Type: {}", output.content_type);
            println!("Final URL: {}", output.final_url);
            println!("Content length: {} chars", output.content.len());
            println!("Truncated: {}", output.truncated);
            println!(
                "Content preview: {}...",
                &output.content.chars().take(200).collect::<String>()
            );

            assert_eq!(output.status, 200);
            assert!(output.content_type.contains("html"));
            assert!(!output.content.is_empty());
            assert!(!output.truncated);
        }
        Err(e) => {
            panic!("test_fetch_html_page FAILED: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_fetch_json_api() {
    let server = create_server();
    let input = WebFetchInput {
        url: "https://httpbin.org/json".to_string(),
        timeout_ms: Some(15000),
        max_length: None,
    };

    let result = server.fetch(input).await;

    match result {
        Ok(output) => {
            println!("=== test_fetch_json_api ===");
            println!("Status: {}", output.status);
            println!("Content-Type: {}", output.content_type);
            println!("Content: {}", output.content);

            assert_eq!(output.status, 200);
            assert!(output.content_type.contains("json"));
            assert!(output.content.contains("slideshow"));
        }
        Err(e) => {
            panic!("test_fetch_json_api FAILED: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_fetch_with_redirect() {
    let server = create_server();
    let input = WebFetchInput {
        url: "https://httpbin.org/redirect/1".to_string(),
        timeout_ms: Some(15000),
        max_length: None,
    };

    let result = server.fetch(input).await;

    match result {
        Ok(output) => {
            println!("=== test_fetch_with_redirect ===");
            println!("Status: {}", output.status);
            println!("Final URL: {}", output.final_url);

            assert_eq!(output.status, 200);
            assert!(output.final_url.contains("get"));
        }
        Err(e) => {
            panic!("test_fetch_with_redirect FAILED: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_fetch_http_upgrades_to_https() {
    let server = create_server();
    let input = WebFetchInput {
        url: "http://example.com".to_string(),
        timeout_ms: None,
        max_length: None,
    };

    let result = server.fetch(input).await;

    match result {
        Ok(output) => {
            println!("=== test_fetch_http_upgrades_to_https ===");
            println!("Final URL: {}", output.final_url);

            assert!(output.final_url.starts_with("https://"));
        }
        Err(e) => {
            panic!("test_fetch_http_upgrades_to_https FAILED: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_fetch_with_max_length_truncation() {
    let server = create_server();
    let input = WebFetchInput {
        url: "https://example.com".to_string(),
        timeout_ms: None,
        max_length: Some(100),
    };

    let result = server.fetch(input).await;

    match result {
        Ok(output) => {
            println!("=== test_fetch_with_max_length_truncation ===");
            println!("Content length: {} chars", output.content.len());
            println!("Truncated: {}", output.truncated);

            assert!(output.truncated);
        }
        Err(e) => {
            panic!("test_fetch_with_max_length_truncation FAILED: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_fetch_invalid_url() {
    let server = create_server();
    let input = WebFetchInput {
        url: "not-a-valid-url".to_string(),
        timeout_ms: None,
        max_length: None,
    };

    let result = server.fetch(input).await;

    match result {
        Ok(_) => {
            panic!("test_fetch_invalid_url FAILED: Should have returned error");
        }
        Err(e) => {
            println!("=== test_fetch_invalid_url ===");
            println!("Error (expected): {:?}", e);
            assert!(e.message.contains("Invalid URL"));
        }
    }
}

#[tokio::test]
async fn test_fetch_nonexistent_domain() {
    let server = create_server();
    let input = WebFetchInput {
        url: "https://this-domain-definitely-does-not-exist-12345.com".to_string(),
        timeout_ms: Some(5000),
        max_length: None,
    };

    let result = server.fetch(input).await;

    match result {
        Ok(_) => {
            panic!("test_fetch_nonexistent_domain FAILED: Should have returned error");
        }
        Err(e) => {
            println!("=== test_fetch_nonexistent_domain ===");
            println!("Error (expected): {:?}", e);
            assert!(
                e.message.contains("Request failed") || e.message.contains("timeout"),
                "Unexpected error: {}",
                e.message
            );
        }
    }
}

// ==================== web_search tests ====================

#[tokio::test]
async fn test_search_basic_query() {
    let server = create_server();
    let input = WebSearchInput {
        query: "rust programming language".to_string(),
        max_results: Some(5),
        allowed_domains: None,
        blocked_domains: None,
    };

    let result = server.search(input).await;

    match result {
        Ok(output) => {
            println!("=== test_search_basic_query ===");
            println!("Provider: {}", output.provider);
            println!("Results count: {}", output.count);
            for (i, r) in output.results.iter().enumerate() {
                println!("[{}] {} - {}", i + 1, r.title, r.url);
                println!("    {}", r.snippet.chars().take(100).collect::<String>());
            }

            assert!(output.count > 0, "Expected at least one result");
            assert!(output.results.len() <= 5);
        }
        Err(e) => {
            panic!("test_search_basic_query FAILED: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_search_with_allowed_domains() {
    let server = create_server();
    let input = WebSearchInput {
        query: "rust programming".to_string(),
        max_results: Some(10),
        allowed_domains: Some(vec![
            "rust-lang.org".to_string(),
            "github.com".to_string(),
        ]),
        blocked_domains: None,
    };

    let result = server.search(input).await;

    match result {
        Ok(output) => {
            println!("=== test_search_with_allowed_domains ===");
            println!("Results count: {}", output.count);
            for r in &output.results {
                println!("{} - {}", r.title, r.url);
                let is_allowed = r.url.contains("rust-lang.org") || r.url.contains("github.com");
                assert!(is_allowed, "URL not in allowed domains: {}", r.url);
            }
        }
        Err(e) => {
            panic!("test_search_with_allowed_domains FAILED: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_search_with_blocked_domains() {
    let server = create_server();
    let input = WebSearchInput {
        query: "programming tutorials".to_string(),
        max_results: Some(10),
        allowed_domains: None,
        blocked_domains: Some(vec!["wikipedia.org".to_string()]),
    };

    let result = server.search(input).await;

    match result {
        Ok(output) => {
            println!("=== test_search_with_blocked_domains ===");
            println!("Results count: {}", output.count);
            for r in &output.results {
                println!("{} - {}", r.title, r.url);
                assert!(
                    !r.url.contains("wikipedia.org"),
                    "Blocked domain found: {}",
                    r.url
                );
            }
        }
        Err(e) => {
            panic!("test_search_with_blocked_domains FAILED: {:?}", e);
        }
    }
}

#[tokio::test]
async fn test_search_query_too_short() {
    let server = create_server();
    let input = WebSearchInput {
        query: "a".to_string(),
        max_results: None,
        allowed_domains: None,
        blocked_domains: None,
    };

    let result = server.search(input).await;

    match result {
        Ok(_) => {
            panic!("test_search_query_too_short FAILED: Should have returned error");
        }
        Err(e) => {
            println!("=== test_search_query_too_short ===");
            println!("Error (expected): {:?}", e);
            assert!(e.message.contains("too short"));
        }
    }
}

#[tokio::test]
async fn test_search_max_results_limit() {
    let server = create_server();
    let input = WebSearchInput {
        query: "software development".to_string(),
        max_results: Some(3),
        allowed_domains: None,
        blocked_domains: None,
    };

    let result = server.search(input).await;

    match result {
        Ok(output) => {
            println!("=== test_search_max_results_limit ===");
            println!("Results count: {}", output.count);

            assert!(output.results.len() <= 3);
        }
        Err(e) => {
            panic!("test_search_max_results_limit FAILED: {:?}", e);
        }
    }
}
