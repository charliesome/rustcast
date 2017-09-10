extern crate ogg;

use std::io;

use self::ogg::PacketReader;
use lewton::VorbisError;
use lewton::inside_ogg::read_headers;
use lewton::audio::{read_audio_packet, PreviousWindowRight, AudioReadError};
use lewton::header::{read_header_comment, IdentHeader, CommentHeader, SetupHeader};

struct NonSeekStream<T: io::Read> {
    stream: T,
}

impl<T> io::Read for NonSeekStream<T> where T: io::Read {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.stream.read(buf) {
            Ok(sz) => Ok(sz),
            Err(e) => Err(e),
        }
    }
}

impl<T> io::Seek for NonSeekStream<T> where T: io::Read {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        panic!("trying to seek NonSeekStream: {:?}", pos);
    }
}

impl<T> NonSeekStream<T> where T: io::Read {
    pub fn new(stream: T) -> NonSeekStream<T> {
        NonSeekStream { stream: stream }
    }
}

#[derive(Debug)]
pub struct Metadata {
    artist: Option<String>,
    title: Option<String>,
}

impl From<CommentHeader> for Metadata {
    fn from(header: CommentHeader) -> Metadata {
        let mut artist = None;
        let mut title = None;

        for (name, value) in header.comment_list {
            match name.as_ref() {
                "ARTIST" => artist = Some(value),
                "TITLE" => title = Some(value),
                _ => (),
            }
        }

        Metadata { artist, title }
    }
}

pub enum OggRead {
    Eof,
    Audio(Vec<Vec<i16>>),
    Metadata(Metadata),
}

pub struct OggStream<T: io::Read> {
    rdr: PacketReader<NonSeekStream<T>>,
    pwr: PreviousWindowRight,

    pub ident_hdr: IdentHeader,
    pub comment_hdr: CommentHeader,
    pub setup_hdr: SetupHeader,
}

impl<T: io::Read> OggStream<T> {
    pub fn new(io: T) -> Result<Self, VorbisError> {
        let mut rdr = PacketReader::new(NonSeekStream::new(io));

        let (ident_hdr, comment_hdr, setup_hdr) = read_headers(&mut rdr)?;

        Ok(OggStream {
            rdr,
            pwr: PreviousWindowRight::new(),
            ident_hdr,
            comment_hdr,
            setup_hdr,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.ident_hdr.audio_sample_rate
    }

    pub fn channels(&self) -> u8 {
        self.ident_hdr.audio_channels
    }

    pub fn read(&mut self) -> Result<OggRead, VorbisError> {
        let packet = match PacketReader::read_packet(&mut self.rdr)? {
            Some(packet) => packet,
            None => return Ok(OggRead::Eof),
        };

        let decoded_packet = read_audio_packet(&self.ident_hdr,
            &self.setup_hdr, &packet.data, &mut self.pwr);

        match decoded_packet {
            Ok(pcm) => return Ok(OggRead::Audio(pcm)),
            Err(AudioReadError::AudioIsHeader) => {
                match read_header_comment(&packet.data) {
                    Ok(comment) => Ok(OggRead::Metadata(comment.into())),
                    Err(e) => Err(e.into()),
                }
            },
            Err(e) => return Err(e.into()),
        }
    }
}