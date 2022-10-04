use opencv::{
    imgcodecs,
    imgproc,
	prelude::*,
	videoio,
    core,
};

use std::{
    path::Path,
    fs,
    io::{prelude::*, stdout, Write, BufReader, BufWriter},
    env,
};
use execute::{
    Execute,
    shell
};
use chrono::{NaiveTime, Timelike};

type Tuple = (f64, f64);

fn main() {
    std::process::Command::new("echo hello > sxx");
    let args: Vec<String> = env::args().collect();
    if args.len() < 4 || args.len() > 6 {
        println!("Invalid arguments.\nUsage: adbreak-rm <video in path> <video out path> <watermark in path> <optional, subtitle in> <optional, subtitle out>");
        return;
    }

    let video_path = &args[1];
    let video_out = &args[2];
    let watermark_path = &args[3];
    let video_name = Path::new(&video_path).file_name().unwrap().to_str().unwrap();
    let video_dir = format!("/tmp/adbreak-rm/{}", video_name);


    let mut vid = VideoData::new();
    vid.get_ad_stamps(video_path, watermark_path);
    if args.len() == 6 {
        vid.new_subs(&args[4], &args[5]);
    }

    

    fs::create_dir_all(&video_dir).unwrap();

    let mut file = fs::File::create(format!("{}/filelist", video_dir)).unwrap();

    let mut ffmpeg_command: String = format!("ffmpeg -i {}", video_path);

    for (i, &(first, second)) in vid.video_stamps.iter().enumerate() {
        //println!("{} {}", first, second);
        let text = format!(" -ss {} -to {} -c copy {}/{}.mp4 -y", first, second, video_dir, i);
        ffmpeg_command.push_str(&text);

        writeln!(file, "file '{}/{}.mp4'", video_dir, i).unwrap();
    }



    let text = format!("\nffmpeg -f concat -safe 0 -i {}/filelist -c copy {} -y", video_dir, video_out);
    ffmpeg_command.push_str(&text);
    shell(&ffmpeg_command).execute_output().unwrap();

    //println!("{}", ffmpeg_command);
    println!("Your new video: {}", video_out); 
    if args.len() == 6 {
        println!("Your new subtitles: {}", &args[5]);
    }
}


pub struct VideoData {
    fps2: f64,
    ad_stamps: Vec<Tuple>,
    video_stamps: Vec<Tuple>,
}
impl VideoData {
    pub fn new() -> Self {
        VideoData { fps2: 0.0, ad_stamps: Vec::new(), video_stamps: Vec::new() }
    }

    fn format_time(ms: u64) -> String {
        let mut ms = ms;

        let mut secs = ms / 1000;
        ms %= 1000;
        
        let mut minutes = secs / 60;
        secs %= 60;

        let hours = minutes / 60;
        minutes %= 60;

        format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, secs, ms)
    }

    fn to_ms(time: String) -> u64 {
        let mut time = time;
        let found = time.matches(':').count();
        if found == 1 { // Minutes : Seconds . Milliseconds
            time = String::from("00:") + &time;
        }
        let t = NaiveTime::parse_from_str(&time, "%H:%M:%S%.3f").unwrap();
        let ms = (t.hour() * 3600000 + t.minute() * 60000 + t.second() * 1000 + t.nanosecond() / 1000000) as u64;
        //println!("{} {} {}", t, Self::format_time(ms), ms);
        ms
    }

    pub fn new_subs(&self, input_filename: &str, output_filename: &str) {
        let in_file = fs::File::open(input_filename).unwrap();
        let out_file = fs::File::create(output_filename).unwrap();

        let reader = BufReader::new(in_file);
        let mut writer = BufWriter::new(out_file);

        for line in reader.lines() {
            let line = line.expect("Can't read line");
            if line.find("-->") != None {
                let times: Vec<&str> = line.split(' ').collect();
                let mut pos1: u64 = Self::to_ms(times[0].to_string())*self.fps2 as u64;
                let mut pos2: u64 = Self::to_ms(times[2].to_string())*self.fps2 as u64;
                let pospos = pos1;

                for v in &self.ad_stamps {
                    if pospos > v.0 as u64 * 1000 {
                        let pos = v.1 as u64 * 1000 - v.0 as u64 * 1000;
                        pos1 -= pos;
                        pos2 -= pos;
                    }
                }
                let newthing = String::from(format!("{} --> {}\n", Self::format_time(pos1/self.fps2 as u64), Self::format_time(pos2/self.fps2 as u64)));
                writer.write(newthing.as_bytes()).unwrap();
            }
            else {
                writer.write(line.as_bytes()).unwrap();
                writer.write(b"\n").unwrap();
            }
        }
    }


    pub fn get_ad_stamps(&mut self, video_path: &str, watermark_path: &str) {
        let mut current_pair: Tuple = (0.0,0.0);
        let mut time_stamps: Vec<Tuple> = Vec::new();
        let mut time_stamps_2: Vec<Tuple> = Vec::new();

        let mut watermark = imgcodecs::imread(watermark_path,0).unwrap();
        imgproc::canny(&watermark.clone(), &mut watermark, 0., 50., 3, false).unwrap();

        let mut vid = videoio::VideoCapture::from_file(video_path,0).unwrap();
        let vid_fps = vid.get(videoio::CAP_PROP_FPS).unwrap();

        //change to bigger to go faster, but lowers accuracy.
        let frame_interval: f64 = vid_fps * 3.0;
        //change to bigger to increase accuracy, but ads might be shown for 1-3 seconds.
        const FRAME_MULTIPLIER: f64 = 7.0;

        let mut frame = core::Mat::default();

        let frame_count = vid.get(videoio::CAP_PROP_FRAME_COUNT).unwrap();

        while vid.get(videoio::CAP_PROP_POS_FRAMES).unwrap() + frame_interval
            < vid.get(videoio::CAP_PROP_FRAME_COUNT).unwrap() {
            if vid.read(&mut frame).unwrap() {
                let curr_frame = vid.get(videoio::CAP_PROP_POS_FRAMES).unwrap();
                let percent = (curr_frame/frame_count*100.0) as u8;
                print!("\r{}% / 100%. or {} frames of {} frames", percent, curr_frame, frame_count);
                stdout().flush().unwrap();

                let result = Self::is_ad(frame.clone(), &watermark);
                if result {
                    //ad
                    if current_pair.0 == 0.0 {
                        current_pair.0 = curr_frame;
                    }
                }
                else {
                    //video
                    if current_pair.0 != 0.0 && current_pair.1 == 0.0 {
                        current_pair.1 = curr_frame;
                    }

                }

                if current_pair.0 > 0.0 && current_pair.1 > 0.0 {
                    if current_pair.1 - current_pair.0 > frame_interval * FRAME_MULTIPLIER {
                        time_stamps.push(current_pair);
                    }
                    current_pair.0 = 0.0;
                    current_pair.1 = 0.0;
                }

                //skip frames to go faster
                vid.set(videoio::CAP_PROP_POS_FRAMES, vid.get(videoio::CAP_PROP_POS_FRAMES).unwrap() + frame_interval).unwrap();
            }
        }


        if current_pair.0 > 0.0 && current_pair.1 > 0.0 && current_pair.1 - current_pair.0 > frame_interval * FRAME_MULTIPLIER {
            time_stamps.push(current_pair);
        }


        //this block here converts ad time stamps to the video's time stamps
        //also convert to seconds from frames
        time_stamps_2.push((0.0,time_stamps.first().unwrap().0/vid_fps));
        for (pos, i) in time_stamps.iter().enumerate().skip(1) {
            let mut temp: Tuple = (0.0, 0.0);
            temp.0 = time_stamps[pos-1].1/vid_fps;
            temp.1 = i.0/vid_fps;
            time_stamps_2.push(temp);
        }
        self.fps2 = vid_fps;
        self.ad_stamps = time_stamps;
        self.video_stamps = time_stamps_2;
    }

    fn is_ad(mut frame: core::Mat, watermark: &core::Mat) -> bool {
        //crop to increase performance. This is good for the sub.png watermark
        //IF YOU HAVE DIFFERENT RESOLUTION THAN 1920x1080 OR DIFFERENT WATERMARK U MUST CHANGE THESE
        frame = core::Mat::roi(&frame.clone(), core::Rect::new(1920-250,40,200,100)).unwrap();

        imgproc::canny(&frame.clone(), &mut frame, 0., 50., 3, false).unwrap();

        let mut result = core::Mat::default();
        unsafe {
            result.create_rows_cols(frame.rows() - watermark.rows() + 1, frame.cols() - watermark.cols() + 1, 0).unwrap();
        }
        imgproc::match_template(&watermark, &frame, &mut result, 1, &core::no_array()).unwrap();

        let (mut min_val, mut max_val) = (0., 0.);
        core::min_max_loc(&result, Some(&mut min_val), Some(&mut max_val), None, None, &core::no_array()).unwrap();

        if min_val as i8 == 1 {
            //ad
            return true;
        }
        //not ad
        false
    }
}
