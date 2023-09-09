pub mod freemind_handler {
    use reqwest::{Client, Response, header::HeaderValue};
    use serde::{Deserialize, Serialize};
    use std::fmt;
    use quick_xml::de::from_str;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    enum FreemindAuth {
        Token,
        Password,
    }

    impl fmt::Display for FreemindAuth {
        fn fmt(&self, f: &mut ::std::fmt::Formatter) -> fmt::Result {
            let displ: &str = match self {
                FreemindAuth::Token => "Token",
                FreemindAuth::Password => "Password",
            };
            write!(f, "{}", displ)
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct FreemindConfig {
        server: String,
        username: String,
        secret: String,
        method: FreemindAuth
    }

    impl ::std::default::Default for FreemindConfig {
        fn default() -> Self {
            Self {
                server: "https://example.com/api:8080".to_string(),
                username: "username".to_string(),
                secret: "password".to_string(),
                method: FreemindAuth::Password
            }
        }
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename = "part")]
    struct Part {
        #[serde(rename = "meta")]
        metadata: Meta,
        #[serde(rename = "data")]
        data: Data,
    }

    #[derive(Serialize, Deserialize)]
    struct Registry {
        #[serde(rename = "entry")]
        entries: Vec<AppElement>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename = "data")]
    struct Data {
        #[serde(rename = "entry")]
        entries: Vec<AppElement>,
    }

    #[derive(Serialize, Deserialize)]
    #[serde(rename = "meta")]
    struct Meta {
        #[serde(rename = "existing_ids")]
        existing_ids: Vec<AppId>,
    }

    #[derive(Serialize, Deserialize)]
    struct AppId {
        id: Vec<u16>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct AppElementTags {
        #[serde(rename = "tag")]
        tags: Vec<String>,
    }

    impl AppElementTags {
        pub fn new(tags: Vec<String>) -> Self {
            Self {
                tags
            }
        }

        pub fn empty() -> Self {
            Self {
                tags: Vec::new(),
            }
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename = "entry")]
    pub struct AppElement {
        #[serde(rename = "@id")]
        id: Option<u16>,
        #[serde(rename = "name")]
        title: String,
        description: String,
        due: Option<u32>,
        tags: Option<AppElementTags>,
    }

    impl PartialEq for AppElement {
        fn eq(&self, other: &AppElement) -> bool {
            match self.id {
                Some(id) => Some(id) == other.id,
                None => self == other, // Isn't this recursive???
            }
        }
    }

    impl AppElement {
        pub fn title(&self) -> &str {
            &self.title
        }
        pub fn description(&self) -> &str {
            &self.description
        }
    }

    #[derive(Debug, Clone)]
    pub struct FreemindState {
        config: FreemindConfig,
        client: Option<Client>,
        elements: Vec<AppElement>,
    }

    impl FreemindState {
        pub fn new(config: FreemindConfig) -> Self {
            Self {
                config,
                client: None,
                elements: Vec::new(),
            }
        }

        pub fn elements(&self) -> &Vec<AppElement> {
            return &self.elements;
        }

        fn handle_empty_client(&mut self) {
            if self.client.is_none() {
                self.client = Some(
                    Client::builder()
                        .user_agent("Freemind Sonos CLI")
                        .build().unwrap()
                );
            }
        }

        pub fn sort_by_due(&mut self) {
            self.elements.sort_by(|a, b| {
                match a.due {
                    Some(due_a) => {
                        match b.due {
                            Some(due_b) => {due_a.cmp(&due_b)},
                            None => {due_a.cmp(&0)}
                        }
                    },
                    None => {
                        match b.due {
                            Some(due_b) => {due_b.cmp(&0)},
                            None => {0.cmp(&0)}
                        }
                    }
                }
            })
        }

        /// Makes a call to the configured server using the provided endpoint
        async fn call(&mut self, endpoint: &str, payload: String) -> Result<Response, reqwest::Error> {
            self.handle_empty_client();
            let res: Response = self.client.as_ref().unwrap()
                .post(format!("{}{}", self.config.server, endpoint))
                .header(
                    "user".to_string(),
                    HeaderValue::from_str(&self.config.username).unwrap()
                )
                .header(
                    format!("{}", &self.config.method).to_lowercase(),
                    &self.config.secret
                )
                .header(
                    "content-type".to_string(),
                    "text/xml".to_string(),
                )
                .body(payload)
                .send()
                .await?;

            Ok(res)
        }

        /// Fetches the whole registry from the server
        pub async fn fetch(&mut self) -> Result<(), reqwest::Error> {
            let res: Response = self.call("/xml/fetch", "".to_string()).await?;

            let headers = res.headers();
            if headers.get("content-type") == Some(&HeaderValue::from_static("text/xml")) {
                let txt = res.text().await?;

                let fetched_registry: Registry = from_str(&txt).unwrap();
                self.elements = fetched_registry.entries;

                self.sort_by_due();
            }

            Ok(())
        }

        /// Fetches all Entries due today
        pub async fn today(&mut self) -> Result<(), reqwest::Error> {
            let res: Response = self.call("/xml/due/today", "".to_string()).await?;

            let headers = res.headers();
            if headers.get("content-type") == Some(&HeaderValue::from_static("text/xml")) {
                let txt = res.text().await?;

                let fetched_part: Part = from_str(&txt).unwrap();
                self.elements = fetched_part.data.entries;

                self.sort_by_due();
            }

            Ok(())
        }


    }


}