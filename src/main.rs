
use rspotify::spotify::client::Spotify;
use rspotify::spotify::util::get_token;
use rspotify::spotify::oauth2::{SpotifyClientCredentials, SpotifyOAuth};

// #[macro_use]
// extern crate serde_derive;

// use rspotify::spotify::model::page::CursorBasedPage;
// use rspotify::spotify::model::playing::PlayHistory;
use rspotify::spotify::model::artist::SimplifiedArtist;
use rspotify::spotify::senum::Country;

use select::document::Document;
//use select::node::Data;
use select::predicate::{Name, Class, And, Comment, Attr};

//use std::io::Read;
use failure::Error;
use failure::err_msg;

use crossbeam_utils::thread::scope;
use std::thread::sleep;
use std::time::Duration;

use reqwest::Error as ReqwestError;

fn main() {

    match run() {
        Ok(()) => {}
        Err(e) => {
            println!("{}", e);
        }
    }

}

fn run() -> Result<(), Error> {

    let mut spotify = connect_to_spotify()?;
    let mut currently_playing_prev = None;
    loop {
        let currently_playing = get_current_song(&mut spotify)?;
        if currently_playing != currently_playing_prev {
            if let Some(ref song) = currently_playing {
                get_and_print_lyrics(&song)
            }
            currently_playing_prev = currently_playing;
        }
        sleep(Duration::from_secs(3));
    }
    //Ok(())
}

fn get_and_print_lyrics(song : &SongInfo) {
    let mut lyrics_from_songtexte = Ok(None);
    let mut lyrics_from_azlyrics = Ok(None);
    scope(|scope| {
        scope.spawn(|| {
            lyrics_from_songtexte = get_lyrics_from_songtextecom(&song);
        });
        scope.spawn(|| {
            lyrics_from_azlyrics = get_lyrics_from_azlyrics(&song);

        });
    });
    println!("");
    println!("");
    println!("=========================================================");
    println!("");
    println!("{}", song.as_string);
    println!("");
    println!("=========================================================");
    println!("");
    if let Ok(Some(l)) = lyrics_from_songtexte {
        println!("{}", l);
    } else if let Ok(Some(l)) = lyrics_from_azlyrics {
        println!("{}", l);
    } else {
        println!("No lyrics found.");
    }
}

fn connect_to_spotify() -> Result<Spotify, Error> {
    let mut oauth = SpotifyOAuth::default()
        .scope("playlist-read-private playlist-read-collaborative streaming user-library-read user-library-modify user-read-private user-top-read user-read-playback-state user-modify-playback-state user-read-currently-playing user-read-recently-played")
        .build();

    match get_token(&mut oauth) {
        Some(token_info) => {
            let client_credential = SpotifyClientCredentials::default()
                .token_info(token_info)
                .build();

            Ok(Spotify::default()
                .client_credentials_manager(client_credential)
                .build())
        }
        None => Err(err_msg("Authentication to Spotify API failed")),
    }
}

#[derive(PartialEq, Debug)]
struct SongInfo {
    songname : String,
    first_artist : String,
    as_string : String,
}

fn get_current_song(spotify : &mut Spotify) -> Result<Option<SongInfo>, Error> {

    let currently = spotify.current_playing(Some(Country::Germany));

    match currently {
        Ok(Some(currently)) => {

            if ! currently.is_playing {
                return Ok(None);
                //println!("IS PLAYING");
            }

            if let Some(track) = currently.item {
                let mut as_string = String::new();
                use std::fmt::Write;
                write!(&mut as_string, "{} from ", track.name);

                let l : &[SimplifiedArtist] = track.artists.as_ref();
                for (i, art) in l.iter().map(|a| &a.name).enumerate() {
                    if i != 0 { as_string += ", "; }
                    as_string += art;
                }

                return Ok( Some( SongInfo {
                    songname : track.name,
                    first_artist : l[0].name.to_owned(),
                    as_string : as_string
                }));
            }
        }
        Err(e) => {
            let err_msg = format!("{}", e);
            if err_msg.contains("Only valid bearer authentication supported") {
                std::mem::replace(spotify, connect_to_spotify()?);
            }
            // match os-error code
            // e is a failure::Error
            match e.downcast_ref()
                   .and_then(|e : &reqwest::Error | e.get_ref() )
                   .and_then(|e : &(dyn std::error::Error + Send + Sync + 'static) | e.downcast_ref())
                   .and_then(|e : &std::io::Error | e.raw_os_error() ) {
                Some(10054) => {
                    std::mem::replace(spotify, connect_to_spotify()?);
                },
                Some(i) => {
                    println!("Other OS Error: {}", i);
                },
                _ => {}
            }

            if err_msg.contains("Eine vorhandene Verbindung") {
                std::mem::replace(spotify, connect_to_spotify()?);
            }
        }
        Ok(None) => {
            println!("current_playing returned Ok(None)");
        }
    }

    Ok(None) // passiert evtl beim Songwechsel
    // return Err(err_msg("There is no song currently playing."));
}



fn get_lyrics_from_azlyrics(song : &SongInfo) -> Result<Option<String>, Error> {

    let mut url : String = "https://search.azlyrics.com/search.php?q=".into();
    url += &song.songname.replace(" ", "+");
    url += "+";
    url += &song.first_artist.replace(" ", "+");
    //println!("{:?}", url);

    let lyricsurl = Document::from_read(reqwest::get(&url)?)?
        .find(And(Name("td"), Class("text-left")))
        .flat_map(|td| td.find(Name("a")))
        .filter_map(|a| a.attr("href"))
        .next()
        .ok_or_else(|| err_msg("No lyrics found"))?
        .to_owned();

    let lyrics = Document::from_read(reqwest::get(&lyricsurl)?)?
        .find(Comment)
        .filter_map(|c| {
            if Some(true) == c.as_comment().map(|c| c.contains("Usage of azlyrics")) {
                c.parent()
            } else { None }
        })
        .map(|div| div.text() )
        .next()
        .ok_or_else(|| err_msg("Lyrics not found") )?;


    Ok(Some(lyrics))
}

fn get_lyrics_from_songtextecom(song : &SongInfo) -> Result<Option<String>, Error> {

    let mut url : String = "http://www.songtexte.com/search?c=songs&q=".into();
    url += &song.songname.replace(" ", "+");
    url += "+";
    url += &song.first_artist.replace(" ", "+");
    //println!("{:?}", url);

    let mut lyricsurl : String = "http://www.songtexte.com/".into();
    lyricsurl += Document::from_read(reqwest::get(&url)?)?
        .find(And(Name("span"), Class("song")))
        .flat_map(|td| td.find(Name("a")))
        .filter_map(|a| a.attr("href"))
        .next()
        .ok_or_else(|| err_msg("No lyrics found"))?;

    let lyrics = Document::from_read(reqwest::get(&lyricsurl)?)?
        .find(And(Name("div"), Attr("id", "lyrics")))
        .map(|div| div.text() )
        .next()
        .ok_or_else(|| err_msg("Lyrics not found") )?;


    Ok(Some(lyrics))
}

