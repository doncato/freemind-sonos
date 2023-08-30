pub mod box_controller {
    use serde::{Deserialize, Serialize};
    use sonor::{args, rupnp::ssdp::URN, RepeatMode, Speaker};
    use std::net::Ipv4Addr;
    use std::fmt;

    const AV_TRANSPORT: &URN = &URN::service("schemas-upnp-org", "AVTransport", 1);
    const DEVICE_PROPERTIES: &URN = &URN::service("schemas-upnp-org", "DeviceProperties", 1);
    const QUEUE: &URN = &URN::service("schemas-sonos-com", "Queue", 1);
    const ZONE_GROUP_TOPOLOGY: &URN = &URN::service("schemas-upnp-org", "ZoneGroupTopology", 1);
    const VIRTUAL_LINE_IN: &URN = &URN::service("schemas-upnp-org", "VirtualLineIn", 1);


    #[derive(Serialize, Deserialize)]
    struct SoundConfig {
        volume: u16,
        crossfade: bool,
        shuffle: bool,
        repeat: bool,
        loudness: bool,
        treble: i8,
        bass: i8,
    }
    impl ::std::default::Default for SoundConfig {
        fn default() -> Self {
            Self {
                volume: 10,
                crossfade: false,
                shuffle: false,
                repeat: false,
                loudness: false,
                treble: 5,
                bass: 5,
            }
        }
    }
    impl fmt::Display for SoundConfig {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", serde_json::to_string(self).unwrap())
        }
}

    #[derive(Serialize, Deserialize)]
    pub struct SpeakerBox {
        ip: Ipv4Addr,
        sound: SoundConfig,
    }
    impl ::std::default::Default for SpeakerBox {
        fn default() -> Self {
            Self {
                ip: Ipv4Addr::new(127, 0, 0, 1),
                sound: SoundConfig::default(),
            }
        }
    }
    impl fmt::Display for SpeakerBox {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", serde_json::to_string(self).unwrap())
        }
    }
    impl SpeakerBox {
        pub fn ip(&self) -> Ipv4Addr {
            return self.ip;
        }

        pub async fn to_speaker(&self) -> Option<Speaker> {
            if let Some(spk) = match Speaker::from_ip(self.ip).await {
                Ok(val) => val,
                Err(err) => {
                    log::error!("{:?}", err);
                    None
                }
            } {
                spk.stop()
                    .await
                    .unwrap_or(log::debug!("Failed to stop playback for {}", self.ip));
                /*
                spk.action(
                    DEVICE_PROPERTIES,
                    "RoomDetectionStartChirping",
                    args! {"Channel": "10", "DurationMilliseconds": "500"},
                )
                .await
                .unwrap();
                */
                spk.set_volume(self.sound.volume)
                    .await
                    .unwrap_or(log::debug!("Failed to set volume for {}", self.ip));
                spk.set_crossfade(self.sound.crossfade)
                    .await
                    .unwrap_or(log::debug!("Failed to set crossfade for {}", self.ip));
                spk.set_shuffle(self.sound.shuffle)
                    .await
                    .unwrap_or(log::debug!("Failed to set shuffle for {}", self.ip));
                spk.set_repeat_mode(if self.sound.repeat {
                    RepeatMode::All
                } else {
                    RepeatMode::None
                })
                .await
                .unwrap_or(log::debug!("Failed to set repeat mode for {}", self.ip));
                spk.set_loudness(self.sound.loudness)
                    .await
                    .unwrap_or(log::debug!("Failed to set loudness for {}", self.ip));
                spk.set_treble(self.sound.treble)
                    .await
                    .unwrap_or(log::debug!("Failed to set treble for {}", self.ip));
                spk.set_bass(self.sound.bass)
                    .await
                    .unwrap_or(log::debug!("Failed to set bass for {}", self.ip));
                spk.clear_queue()
                    .await
                    .unwrap_or(log::debug!("Failed to clear playlist for {}", self.ip));

                if let Ok(response) = spk
                    .action(
                        AV_TRANSPORT,
                        "BecomeCoordinatorOfStandaloneGroup",
                        args! { "InstanceID": "0" },
                    )
                    .await
                {
                    log::info!("{:?}", response);
                } else {
                    log::error!("Failed to set Coordinator for {}", self.ip);
                }
                /*
                spk.action(
                    QUEUE,
                    "CreateQueue",
                    args! { "QueueOwnerID": "RINCON_949F3E77CCD201400", "QueueOwnerContext": "THIS_QUEUE", "QueuePolicy": "0"},
                )
                .await
                .unwrap();
                */
                Some(spk)
            } else {
                log::warn!("Failed to connect to {}", self.ip);
                None
            }
        }
    }
}