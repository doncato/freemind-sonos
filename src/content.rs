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
