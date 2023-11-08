pub mod freemind_handler {
    use cron::Schedule;
    use chrono::{Local, TimeZone};
    use reqwest::{Client, Response, header::HeaderValue};
    use serde::{Deserialize, Serialize};
    use std::cmp::{min, Ordering};
    use std::fmt;
    use std::str::FromStr;
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

    #[derive(Eq, PartialEq, Debug, Clone, Serialize, Deserialize)]
    pub struct Preparation {
        description: Option<String>,
        time: Option<u32>,
    }

    #[derive(Eq, Debug, Clone, Serialize, Deserialize)]
    #[serde(rename = "entry")]
    pub struct AppElement {
        #[serde(skip)]
        takes_place_on: Option<u32>,
        #[serde(rename = "@id")]
        id: Option<u16>,
        #[serde(rename = "name")]
        title: String,
        description: String,
        due: Option<u32>,
        repeats: Option<String>,
        preparation: Option<Preparation>,
        location: Option<String>,
        alert: Option<String>,
    }

    impl PartialOrd for AppElement {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    impl Ord for AppElement {
        fn cmp(&self, other: &Self) -> Ordering {
            self.takes_place_on.cmp(&other.takes_place_on)
        }
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
        pub fn location(&self) -> &str {
            match &self.location {
                Some(val) => &val,
                None => &""
            }
        }

        pub fn description(&self) -> &str {
            &self.description
        }

        pub fn timepoint(&self) -> Option<String> {
            if let Some(mut takes_place) = self.takes_place_on {
                if let Some(prepare) = &self.preparation {
                    if let Some(prep) = prepare.time {
                        takes_place += prep*60;
                    }
                }

                /*
                return Some(format!("{}", chrono::Utc.timestamp_opt(takes_place as i64, 0)
                    .unwrap()
                    .with_timezone(&Local)
                    .format("%H:%M")
                ));
                */
                return match chrono::Utc.timestamp_opt(takes_place as i64, 0) {
                    chrono::LocalResult::None => None,
                    chrono::LocalResult::Single(val) => Some(val.with_timezone(&chrono::Local).format("%H:%M").to_string()),
                    chrono::LocalResult::Ambiguous(val, _) => Some(val.with_timezone(&chrono::Local).format("%H:%M").to_string()),
                };
            }
            None
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
                        .use_rustls_tls()
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

        /// Computes for every element the actual time in which it takes place
        fn compute_takes_place(&mut self) {
            self.elements
                .iter_mut()
                .for_each(|e: &mut AppElement| {
                    let mut due = e.due;

                    if let Some(repeat) = &e.repeats {
                        if let Ok(schedule) = Schedule::from_str(repeat) {
                            if let Some(next_occasion) = schedule.upcoming(Local).next() {
                                let next_due: u32 = next_occasion.naive_utc().and_utc().timestamp().try_into().unwrap_or(u32::MAX);
                                due = Some(min(due.unwrap_or(u32::MAX), next_due));
                            };
                        };
                    };

                    if due.is_some() {
                        if let Some(prep) = &e.preparation {
                            if let Some(delta) = prep.time {
                                due = Some(due.unwrap() - delta*60);
                            }
                        }
                    }
                    e.takes_place_on = due;
                });
        }

        /// Determines whether an alert should be triggered or not.
        pub fn needs_trigger(&mut self, interval: u16) -> bool {
            let now: u32 = chrono::offset::Local::now()
                .naive_utc()
                .timestamp()
                .try_into()
                .unwrap_or(0);

            self.compute_takes_place();

            self.elements()
                .iter()
                .filter(|e| e.takes_place_on.is_some() && e.alert.is_some())
                .filter(|e| {
                    e.takes_place_on.unwrap_or(0) >= now &&
                    e.takes_place_on.unwrap_or(0) < (now + (interval*60) as u32)
                })
                .next()
                .is_some()
        }

        /// Parses the available information and returns all Elements that take place today
        /// and sorts them when they occur
        pub fn get_today(&mut self) -> Vec<&AppElement> {
            let mut result: Vec<&AppElement> = Vec::new();

            let now: chrono::DateTime<Local> = chrono::offset::Local::now();

            let today_start: u32 = now
                //.date_naive()
                //.and_hms_opt(0,0,0)
                //.unwrap()
                //.and_local_timezone(Local)
                //.unwrap()
                .naive_utc()
                .and_utc()
                .timestamp()
                .try_into()
                .unwrap_or(0);
            let today_end: u32 = now
                .date_naive()
                .and_hms_opt(23, 59, 59)
                .unwrap()
                .and_local_timezone(Local)
                .unwrap()
                .naive_utc()
                .and_utc()
                .timestamp()
                .try_into()
                .unwrap_or(u32::MAX);

            self.compute_takes_place();

            result = self.elements()
                .iter()
                .filter(|e| e.takes_place_on.is_some())
                .filter(|e| {
                    e.takes_place_on.unwrap_or(0) >= today_start &&
                    e.takes_place_on.unwrap_or(0) <= today_end
                })
                .collect();

            result.sort();

            return result;
        }

        /*
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
        */


    }


}