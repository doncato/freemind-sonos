pub mod speech {
    use chrono::{Duration, NaiveTime, Utc};
    use reqwest;
    use bytes::Bytes;



    pub async fn get_speech_voicerss(text: &str, tts_api_key: &str) -> Result<Bytes, reqwest::Error> {
        let language = "en-gb";
        return reqwest::get(format!(
            "http://api.voicerss.org/?key={}&hl={}&c=MP3&f=48khz_16bit_stereo&v=Nancy&src={}",
            tts_api_key, language, text
        ))
        .await?
        .bytes()
        .await;
    }

    fn get_daytime_from_time<'t>(time: NaiveTime) -> &'t str {
        if time > NaiveTime::from_hms(18, 30, 0) {
            "Evening"
        } else if time > NaiveTime::from_hms(15, 30, 0) {
            "Afternoon"
        } else if time > NaiveTime::from_hms(11, 30, 0) {
            "Noon"
        } else if time > NaiveTime::from_hms(5, 30, 0) {
            "Morning"
        } else {
            "Tag"
        }
    }

    pub async fn get_date(user: String, timezone: i8, tts_api_key: &str) -> Result<Bytes, reqwest::Error> {
        let time = Utc::now()
            .checked_add_signed(Duration::hours(timezone as i64))
            .unwrap_or(Utc::now());
        let text = format!(
            "Good {} {}. \n Today is {}, the {} {} {}. \n The time is {}.",
            get_daytime_from_time(time.time()),
            user,
            &time.format("%A").to_string(),
            time.format("%d"),
            &time.format("%B").to_string(),
            time.format("%Y"),
            time.format("%H:%M")
        );
        get_speech_voicerss(&text, tts_api_key).await
    }
}

pub mod music {
    use std::fmt;

    use rand::seq::SliceRandom;
    use serde::{Deserialize, Serialize};
    use reqwest;

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    pub struct Title {
        pub name: String,
        pub id: String,
        #[serde(rename = "AlbumArtist")]
        pub artist: String,
        pub album: String
    }

    impl fmt::Display for Title {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", serde_json::to_string(self).unwrap())
        }
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    #[serde(rename_all = "PascalCase")]
    pub struct Playlist {
        items: Vec<Title>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize)]
    pub struct JellyfinConfig {
        server: String,
        api_key: String,
        playlist_id: String,
        user_id: String,
    }

    impl ::std::default::Default for JellyfinConfig {
        fn default() -> Self {
            Self {
                server: "https://example.com/".to_string(),
                api_key: "YOUR API KEY".to_string(),
                playlist_id: "id of the playlist to use".to_string(),
                user_id: "id of the user".to_string(),
            }
        }
    }

    pub async fn get_random_jellyfin_track(config: &JellyfinConfig) -> Result<Option<Title>, reqwest::Error> {
        let client: reqwest::Client = reqwest::Client::new();
        let playlist: Playlist = client.get(
            format!(
                "{}/Playlists/{}/Items?api_key={}&userId={}",
                config.server,
                config.playlist_id,
                config.api_key,
                config.user_id
            )
        )
            .send()
            .await?
            .json::<Playlist>()
            .await?;

        return match playlist.items.choose(&mut rand::thread_rng()) {
            Some(title) => Ok(Some(title.clone())),
            None => Ok(None)
        }
    }
}
