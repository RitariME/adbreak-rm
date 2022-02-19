use opencv::{
    imgcodecs,
    imgproc,
	highgui,
	prelude::*,
	Result,
	videoio,
    core,
};

type Tuple = (usize, usize);
const FRAME_INTERVAL: usize = 60;

fn main() -> Result<()> {
    //highgui::named_window("lol", 0)?;
    let result: Vec<Tuple> = get_ad_stamps("/mnt/hdd/projekt/rm-adbreak/out.mp4",
                               "/mnt/hdd/projekt/rm-adbreak/sub.png").unwrap();

    for &(first, second) in result.iter() {
        println!("{} {}", first, second);
    }

    highgui::wait_key(100000)?;
    Ok(())
}

fn get_ad_stamps(video_path: &str, logo_path: &str) -> Result<Vec<Tuple>> {
    let mut current_pair: Tuple = (0,0);
    let mut time_stamps: Vec<Tuple> = Vec::new();

    let mut logo = imgcodecs::imread(logo_path,0)?;
    imgproc::canny(&logo.clone(), &mut logo, 0., 50., 3, false)?;

    let mut vid = videoio::VideoCapture::from_file(video_path,0)?;
    let mut frame = core::Mat::default();
    while vid.get(videoio::CAP_PROP_POS_FRAMES)? as usize + FRAME_INTERVAL < vid.get(videoio::CAP_PROP_FRAME_COUNT)? as usize {
        if vid.read(&mut frame)? {
            let curr_frame = vid.get(videoio::CAP_PROP_POS_FRAMES)?;
            println!("{} {}", curr_frame, vid.get(videoio::CAP_PROP_FRAME_COUNT)?);

            let result = is_ad(frame.clone(), &logo);
            if result.unwrap() {
                //mainos
                if current_pair.0 == 0 {
                    current_pair.0 = curr_frame as usize;
                }
            }
            else {
                //video
                if current_pair.0 != 0 && current_pair.1 == 0 {
                    current_pair.1 = curr_frame as usize;
                }

            }
            //println!("{} {}", current_pair.0, current_pair.1);
            if current_pair.0 > 0 && current_pair.1 > 0 {
                if current_pair.1 - current_pair.0 > FRAME_INTERVAL*10 {
                    //println!("{} {}", current_pair.0, current_pair.1);
                    time_stamps.push(current_pair);
                    current_pair.0 = 0;
                    current_pair.1 = 0;
                }
                else {
                    current_pair.0 = 0;
                    current_pair.1 = 0;
                }
            }
            vid.set(videoio::CAP_PROP_POS_FRAMES, vid.get(videoio::CAP_PROP_POS_FRAMES)? + FRAME_INTERVAL as f64)?;
        }
    }


    if current_pair.0 > 0 && current_pair.1 > 0 && current_pair.1 - current_pair.0 > FRAME_INTERVAL*10 {
        time_stamps.push(current_pair);
    }

    println!("DONE");
    Ok(time_stamps)
}

fn is_ad(mut frame: core::Mat, logo: &core::Mat) -> Result<bool> {
    frame = core::Mat::roi(&frame.clone(), core::Rect::new(1920-250,40,200,100))?;

    imgproc::canny(&frame.clone(), &mut frame, 0., 50., 3, false)?;

    let mut result = core::Mat::default();
    unsafe {
        result.create_rows_cols(frame.rows() - logo.rows() + 1, frame.cols() - logo.cols() + 1, 0)?;
    }
    imgproc::match_template(&logo, &frame, &mut result, 1, &core::no_array())?;
    //core::normalize(&result.clone(), &mut result, 0.0, 1.0, core::NORM_MINMAX, -1, &core::no_array())?;


    let (mut min_val, mut max_val) = (0., 0.);
    core::min_max_loc(&result, Some(&mut min_val), Some(&mut max_val), None, None, &core::no_array())?;
    //highgui::imshow("lol", &frame)?;
    //highgui::wait_key(1)?;

    //println!("{}", min_val);

    let mut ret_val = false;
    if min_val as i32 == 1 {
        ret_val = true;
        //ad
    }
    Ok(ret_val)
}
