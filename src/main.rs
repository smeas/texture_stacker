#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(unreachable_code)]

use png::{BitDepth, ColorType};
use serde::Deserialize;
use std::{
    borrow::Borrow,
    collections::HashMap,
    env,
    error::Error,
    ffi::OsStr,
    fs,
    fs::File,
    io::BufWriter,
    io::{self, Write},
    path::{Path, PathBuf},
    process::exit,
    str::FromStr,
    time::Instant,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

struct RawImage {
    data: Vec<u8>,
    format: ImageFormat,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct ImageFormat {
    width: u32,
    height: u32,
    color_type: ColorType,
    bit_depth: BitDepth,
}

#[derive(Debug)]
struct InputTextureSet {
    name: String,
    textures: Vec<Option<String>>,
}

#[derive(Debug, Deserialize, Default)]
struct ConfigFile {
    #[serde(default)]
    keep_mask_alpha: bool,

    #[serde(default)]
    suffixes: Vec<String>,

    #[serde(default)]
    output_masks: bool,

    input_directory: Option<String>,
    output_texture_name: Option<String>,
}

#[derive(Debug)]
struct Config {
    keep_mask_alpha: bool,
    suffixes: Vec<String>,
    output_masks: bool,
    output_directory: String,
    output_texture_name: String,
}

fn read_image_from_file(file_name: &str) -> Result<RawImage> {
    let infile = File::open(&file_name)?;
    let decoder = png::Decoder::new(infile);

    let mut reader = decoder.read_info()?;
    let mut buffer = vec![0; reader.output_buffer_size()];

    let info = reader.next_frame(&mut buffer)?;
    let bytes = &buffer[..info.buffer_size()];

    if reader.info().is_animated() {
        panic!("APNG is not supported");
    }

    Ok(RawImage {
        data: bytes.to_vec(),
        format: ImageFormat {
            width: info.width,
            height: info.height,
            bit_depth: info.bit_depth,
            color_type: info.color_type,
        },
    })
}

fn write_image_to_file(file_name: &str, image: &RawImage) -> Result<()> {
    let format = &image.format;

    let file = File::create(&file_name)?;
    let mut w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(&mut w, format.width, format.height);
    encoder.set_depth(format.bit_depth);
    encoder.set_color(format.color_type);
    //encoder.set_srgb(png::SrgbRenderingIntent::)

    let mut writer = encoder.write_header()?;
    writer.write_image_data(&image.data)?;

    Ok(())
}

///
/// Calculate the amount of bytes per pixel of the given image format.
///
/// Panics if the format is not supported.
///
fn calc_pixel_stride(format: &ImageFormat) -> usize {
    match format {
        ImageFormat {
            bit_depth: BitDepth::Eight,
            color_type: ColorType::Rgb,
            ..
        } => 3,
        ImageFormat {
            bit_depth: BitDepth::Eight,
            color_type: ColorType::Rgba,
            ..
        } => 4,
        ImageFormat {
            bit_depth: BitDepth::Sixteen,
            color_type: ColorType::Rgb,
            ..
        } => 6,
        ImageFormat {
            bit_depth: BitDepth::Sixteen,
            color_type: ColorType::Rgba,
            ..
        } => 8,
        _ => panic!(),
    }
}

#[derive(PartialEq, Eq)]
struct Pixel(u8, u8, u8, u8);

///
/// Interprets a slice of bytes as a pixel in the given image format.
///
/// Panics if the format is not supported.
///
fn bytes_to_pixel(slice: &[u8], format: &ImageFormat) -> Pixel {
    match format {
        ImageFormat {
            bit_depth: BitDepth::Eight,
            color_type: ColorType::Rgb,
            ..
        } => Pixel(slice[0], slice[1], slice[2], 255),
        ImageFormat {
            bit_depth: BitDepth::Eight,
            color_type: ColorType::Rgba,
            ..
        } => Pixel(slice[0], slice[1], slice[2], slice[3]),
        _ => panic!(),
    }
}

///
/// Writes the byte representation of pixel in the given format to the given slice.
///
/// Panics if the format is not supported.
///
fn pixel_to_bytes(pixel: Pixel, format: &ImageFormat, slice: &mut [u8]) {
    match format {
        ImageFormat {
            bit_depth: BitDepth::Eight,
            color_type: ColorType::Rgb,
            ..
        } => {
            slice[0] = pixel.0;
            slice[1] = pixel.1;
            slice[2] = pixel.2;
        }
        ImageFormat {
            bit_depth: BitDepth::Eight,
            color_type: ColorType::Rgba,
            ..
        } => {
            slice[0] = pixel.0;
            slice[1] = pixel.1;
            slice[2] = pixel.2;
            slice[3] = pixel.3;
        }
        _ => panic!(),
    }
}

fn create_mask_from_alpha_channel(image: &RawImage) -> Vec<bool> {
    let format = &image.format;

    assert_eq!(format.color_type, ColorType::Rgba);

    let pixel_stride = calc_pixel_stride(format);
    let num_pixels = format.width as usize * format.height as usize;

    assert_eq!(image.data.len(), num_pixels * pixel_stride);

    let mut pixel_mask = vec![false; num_pixels];

    for i in 0..num_pixels {
        let pixel = bytes_to_pixel(&image.data[i * pixel_stride..], format);
        pixel_mask[i] = pixel.3 != 0;
    }

    pixel_mask
}

fn combine_texture_sets(input_sets: &[InputTextureSet], config: &Config) -> Result<()> {
    // Assumptions.
    for texture_set in input_sets {
        assert!(texture_set.textures.len() > 0);
        assert!(texture_set.textures[0].is_some());
    }

    // Pixel mask for each texture set.
    let mut set_masks = vec![];
    let mut working_res = (0u32, 0u32);

    // Compute masks for each texture set.
    for input_set in input_sets {
        let image = read_image_from_file(input_set.textures[0].as_ref().unwrap())?;

        assert_eq!(image.format.color_type, ColorType::Rgba); // need alpha channel for mask
        assert!(image.format.width > 0 && image.format.height > 0);

        if working_res == (0, 0) {
            working_res = (image.format.width, image.format.height);
        } else {
            assert_eq!(working_res, (image.format.width, image.format.height));
        }

        let mask = create_mask_from_alpha_channel(&image);
        set_masks.push(mask);
    }

    if config.output_masks {
        // Write masks to files for debugging.
        for (i, mask) in set_masks.iter().enumerate() {
            let num_pixels = mask.len();
            let format = ImageFormat {
                width: working_res.0,
                height: working_res.1,
                bit_depth: BitDepth::Eight,
                color_type: ColorType::Rgba,
            };
            let stride = calc_pixel_stride(&format);
            let mut buffer = vec![0u8; num_pixels * stride];

            for i in 0..num_pixels {
                let pixel = if mask[i] {
                    Pixel(255, 255, 255, 255)
                } else {
                    Pixel(0, 0, 0, 255)
                };
                pixel_to_bytes(pixel, &format, &mut buffer[i * stride..]);
            }

            let filename = format!("{}/mask{}.png", config.output_directory, i);
            write_image_to_file(
                &filename,
                &RawImage {
                    data: buffer,
                    format,
                },
            )
            .unwrap_or_else(|err| {
                eprintln!(
                    "[ERROR] Failed to write mask to file '{}': {:?}",
                    filename, err
                )
            });
        }
    }

    // Combine all the image sets into the output files.
    for (suffix_index, suffix) in config.suffixes.iter().enumerate() {
        let mut output_image: Option<RawImage> = None;
        let mut first = true;

        for (set_index, input_set) in input_sets.iter().enumerate() {
            // Grab the texture filename if it exists.
            let texture_filename;
            if let Some(filename) = &input_set.textures[suffix_index] {
                texture_filename = filename;
            } else {
                continue;
            }

            let image = read_image_from_file(texture_filename)?;
            let format = &image.format;
            let is_mask_source_image = suffix_index == 0;
            //const KEEP_MASK_SOURCE_ALPHA: bool = false;

            if let Some(raw_output_image) = &output_image {
                // Output image has already been created. Validate the current image's format
                // against the output image's one.

                let output_format = &raw_output_image.format;

                assert_eq!(output_format.width, format.width);
                assert_eq!(output_format.height, format.height);
                assert_eq!(output_format.bit_depth, format.bit_depth);

                if format.color_type != output_format.color_type {
                    match (format.color_type, output_format.color_type) {
                        (ColorType::Rgb, ColorType::Rgba) => {} // ok
                        (ColorType::Rgba, ColorType::Rgb) => {
                            // alpha is lost
                            if is_mask_source_image && !config.keep_mask_alpha {
                                // ok: desired behaviour
                            } else {
                                eprintln!("[WARN] Encountered an unexpected alpha channel in {}, it will be discarded as the previous texture(s) did not have one.", texture_filename);
                            }
                        }
                        _ => {
                            assert!(false, "mismatched color format");
                        }
                    }
                }
            } else {
                // Create the output image using the format of the current image.

                let mut output_format = *format;
                if is_mask_source_image && !config.keep_mask_alpha {
                    output_format.color_type = ColorType::Rgb;
                }

                let buffer_size = format.width as usize
                    * format.height as usize
                    * calc_pixel_stride(&output_format);

                output_image = Some(RawImage {
                    data: vec![0; buffer_size],
                    format: output_format,
                });
            }

            if first {
                // For the first image in the set we just copy the image without masking to get a nice background color for the output image.
                copy_image(&image, output_image.as_mut().unwrap());
                first = false;
            } else {
                let mask = &set_masks[set_index];
                copy_image_masked(&image, output_image.as_mut().unwrap(), &mask);
            }
        }

        if let Some(image) = &output_image {
            let mut output_file_path = PathBuf::new();
            output_file_path.push(&config.output_directory);
            // NOTE: If output_texture_name contains a '/' or '\', this could lead to unexpected results.
            output_file_path.push(format!("{}{}", &config.output_texture_name, suffix));
            output_file_path.set_extension("png");

            let output_file = output_file_path.to_str().unwrap();
            println!("{}", output_file);
            write_image_to_file(output_file, &image)?;
        }
    }

    Ok(())
}

fn copy_image_masked(source_image: &RawImage, dest_image: &mut RawImage, mask: &[bool]) {
    assert_eq!(source_image.format.width, dest_image.format.width);
    assert_eq!(source_image.format.height, dest_image.format.height);

    let num_pixels = source_image.format.width as usize * source_image.format.height as usize;
    let source_stride = source_image.data.len() / num_pixels;
    let dest_stride = dest_image.data.len() / num_pixels;

    assert_eq!(mask.len(), num_pixels);

    for i in 0..num_pixels {
        if mask[i] {
            let pixel = bytes_to_pixel(
                &source_image.data[i * source_stride..],
                &source_image.format,
            );
            pixel_to_bytes(
                pixel,
                &dest_image.format,
                &mut dest_image.data[i * dest_stride..],
            );
        }
    }
}

fn copy_image(source_image: &RawImage, dest_image: &mut RawImage) {
    if source_image.format == dest_image.format {
        // Fast path: same format, just a memcpy.
        assert_eq!(source_image.data.len(), dest_image.data.len());
        dest_image.data.copy_from_slice(&source_image.data);
    } else {
        // Slow path: different format, need to convert.
        assert_eq!(source_image.format.width, dest_image.format.width);
        assert_eq!(source_image.format.height, dest_image.format.height);

        let num_pixels = source_image.format.width as usize * source_image.format.height as usize;
        let source_stride = source_image.data.len() / num_pixels;
        let dest_stride = dest_image.data.len() / num_pixels;

        for i in 0..num_pixels {
            let pixel = bytes_to_pixel(
                &source_image.data[i * source_stride..],
                &source_image.format,
            );
            pixel_to_bytes(
                pixel,
                &dest_image.format,
                &mut dest_image.data[i * dest_stride..],
            );
        }
    }
}

fn suffix_from_filename(filename: &str) -> Option<&str> {
    let path: &Path = filename.as_ref();

    if let Some(stem) = path.file_stem() {
        let stem = stem.to_str().unwrap();
        if let Some(pos) = stem.rfind('_') {
            return Some(&stem[pos..]);
        }
    }

    None
}

fn collect_and_group_files_by_name<P: AsRef<Path>>(
    directory: &P,
) -> Result<HashMap<String, Vec<String>>> {
    let directory = directory.as_ref();
    assert!(directory.is_dir()); // TODO

    let mut map = HashMap::<String, Vec<String>>::new();

    // TODO: Iterate over files sorted to get a consistent result.
    for entry in directory.read_dir()? {
        let entry = entry?;
        let path = entry.path();

        if path.extension() != Some("png".as_ref()) {
            continue;
        }

        if let Some(stem) = path.file_stem() {
            let stem = stem.to_string_lossy();

            if let Some(pos) = stem.rfind('_') {
                let pre = &stem[..pos];

                match map.get_mut(pre) {
                    Some(vec) => {
                        vec.push(path.to_string_lossy().to_string());
                    }
                    None => {
                        let mut vec = Vec::new();
                        vec.push(path.to_string_lossy().to_string());
                        map.insert(pre.to_string(), vec);
                    }
                }
            }
        }
    }

    Ok(map)
}

fn gather_texture_sets_from_directory<P, S>(
    path: &P,
    suffixes: &[S],
) -> Result<Vec<InputTextureSet>>
where
    P: AsRef<Path>,
    S: AsRef<str>,
{
    let files = collect_and_group_files_by_name(path)?;
    let mut output: Vec<InputTextureSet> = Vec::new();

    for (name, textures) in &files {
        let mut texture_set = InputTextureSet {
            name: name.clone(),
            textures: vec![None; suffixes.len()],
        };

        for (i, suffix) in suffixes.iter().enumerate() {
            if let Some(file) = textures
                .iter()
                .find(|filename| suffix_from_filename(filename) == Some(suffix.as_ref()))
            {
                texture_set.textures[i] = Some(file.clone());
            }
        }

        output.push(texture_set);
    }

    Ok(output)
}

fn get_config() -> Result<ConfigFile> {
    let path: &Path = "config.toml".as_ref();
    if path.is_file() {
        let raw = fs::read_to_string(path)?;
        Ok(toml::from_str(&raw)?)
    } else {
        Ok(Default::default())
    }
}

fn prompt_for_string(prompt: &str) -> Result<String> {
    print!("{}", prompt);
    io::stdout().flush()?;
    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    Ok(buf.trim().to_owned())
}

fn is_directory(path: &impl AsRef<Path>) -> bool {
    path.as_ref().is_dir()
}

fn main() {
    let argv: Vec<String> = env::args().collect();
    let config_file: ConfigFile = get_config().expect("failed to read config file");

    if config_file.suffixes.len() == 0 {
        eprintln!("[ERROR] No suffixes specified in config.");
        exit(1);
    }

    // input_directory = config > args > prompt
    let input_directory = config_file.input_directory.unwrap_or_else(|| {
        if argv.len() > 1 {
            argv[1].clone()
        } else {
            prompt_for_string("Input directory? ").unwrap()
        }
    });

    if !is_directory(&input_directory) {
        eprintln!("[ERROR] The specified input directory is not valid.");
        exit(1);
    }

    // output_texture_name = config > prompt
    // Can be a relative path, so has to be unpacked appropriately.
    let mut output_texture_name = config_file
        .output_texture_name
        .unwrap_or_else(|| prompt_for_string("Output texture name (relative to input directory)? ").unwrap())
        // Make sure it does not start with a slash, as that could cause paths to be overwritten by an absolute later on.
        .trim_start_matches(&['/', '\\'])
        .to_owned();

    let output_directory = {
        let mut output_directory_path = PathBuf::new();
        output_directory_path.push(&input_directory);

        let empty_path: &Path = "".as_ref();
        let output_texture_path: &Path = output_texture_name.as_ref();
        let parent_dir = output_texture_path.parent().unwrap_or(empty_path);
        if parent_dir != empty_path {
            output_directory_path.push(&parent_dir);
            fs::create_dir_all(&output_directory_path).unwrap();

            output_texture_name = output_texture_path
                .file_name()
                .map(|s| s.to_str().unwrap())
                .unwrap_or("")
                .to_owned();
        }

        output_directory_path.to_str().unwrap().to_owned()
    };

    let config = Config {
        suffixes: config_file.suffixes,
        keep_mask_alpha: config_file.keep_mask_alpha,
        output_masks: config_file.output_masks,
        output_directory,
        output_texture_name,
    };

    //dbg!(&config);

    let mut texture_sets =
        gather_texture_sets_from_directory(&input_directory, &config.suffixes).unwrap();

    // Remove invalid texture sets from the list.
    texture_sets.retain(|set| {
        // Make sure the first texture type is given as this will be used for the mask.
        let valid = set.textures.len() > 0 && set.textures[0].is_some();
        if !valid {
            eprintln!(
                "Unable to compute mask for texture set '{}' because the first texture type '{}' is missing. This texture set will be skipped.",
                set.name,
                config.suffixes[0]);
        }

        valid
    });

    let start_time = Instant::now();

    combine_texture_sets(&texture_sets, &config).unwrap();

    println!("Finished in {} s", start_time.elapsed().as_secs_f32());

    prompt_for_string("Press enter to close this window...").unwrap();
}
