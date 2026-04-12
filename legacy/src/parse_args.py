import argparse


def parse_program_args():
    parser = argparse.ArgumentParser(
        prog='LFP-Media-Compressor',
        description='Compress all media in a folder',
    )
    # Standard options
    parser.version = "03.00"
    parser.add_argument('-v', '--verbose', action='store_true')  # on/off flag
    parser.add_argument('--version', action='version')

    # General arguments
    parser.add_argument('folder_path')  # positional argument
    parser.add_argument('-m', '--mode', choices=["all", "videos", "images", "fixdates"], default="all")
    parser.add_argument('--replace', help="replace input files with output files (possibly modified file extension)")
    parser.add_argument('--only', help="only process a specific filename")
    parser.add_argument('--ignore-timestamps', help="ignore missing or mismatching file creation timestamps")

    # Video-Options
    video_options = parser.add_argument_group("Video Options")
    video_options.add_argument('-crf', '--crf', type=int,
                               default=28,
                               help="CRF target, see https://trac.ffmpeg.org/wiki/Encode/H.265#Ratecontrolmodes")
    video_options.add_argument('--ffmpeg-speed',
                               default="faster",
                               choices=["ultrafast", "superfast", "veryfast", "faster", "fast",
                                        "medium", "slow", "slower", "veryslow", "placebo"],
                               help="FFmpeg speed profile, see https://x265.readthedocs.io/en/master/presets.html")
    video_options.add_argument('--video-max-pixels', type=int,
                               default=0,
                               help="Resize to this maximum amount of image-pixels in any dimension. "
                                    "Set to 0 to disable. (Default 0)")

    # Image-Options
    image_options = parser.add_argument_group("Image Options")
    image_options.add_argument('--webpq', type=int,
                               default=45,
                               help="WebP image quality setting.")
    image_options.add_argument('--image-max-pixels', type=int,
                               default=1600,
                               help="Resize to this maximum amount of image-pixels in any dimension")

    args = parser.parse_args()
    return args
