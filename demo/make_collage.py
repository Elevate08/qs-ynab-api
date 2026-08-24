#!/usr/bin/env python3
"""Build a preview collage from YNAB Pulse screenshots.

Arranges the 6 plugin view screenshots in a clean 3x2 grid on a themed background
with the "YNAB Pulse" header, subtitle, and individual card badges.
"""

import os
import sys
from PIL import Image, ImageDraw, ImageFont, ImageFilter

REPO = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SCREENSHOTS_DIR = os.path.join(REPO, "docs", "screenshots")
OUTPUT_PATH = os.path.join(REPO, "preview.png")

SHOTS = [
    ("01-buckets.png", "Budget Buckets & Overspent Alerts"),
    ("02-income-and-trends.png", "Age of Money & 6-Mo Trends"),
    ("03-spending-analysis.png", "Interactive Spending Breakdown"),
    ("04-spending-drilldown.png", "Sub-Category Spending Details"),
    ("05-settings.png", "Multi-Budget & Refresh Controls"),
    ("06-onboarding.png", "Secure Keyring Token Setup"),
]

def load_font(size, bold=False):
    font_candidates = [
        "/usr/share/fonts/noto/NotoSans-Bold.ttf" if bold else "/usr/share/fonts/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/Adwaita/AdwaitaMono-Bold.ttf" if bold else "/usr/share/fonts/Adwaita/AdwaitaMono-Regular.ttf",
        "/usr/share/fonts/TTF/DejaVuSans-Bold.ttf" if bold else "/usr/share/fonts/TTF/DejaVuSans.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf" if bold else "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ]
    for path in font_candidates:
        if os.path.isfile(path):
            try:
                return ImageFont.truetype(path, size)
            except Exception:
                continue
    return ImageFont.load_default()

def round_corners(im, rad):
    mask = Image.new("L", im.size, 0)
    draw = ImageDraw.Draw(mask)
    draw.rounded_rectangle([(0, 0), im.size], radius=rad, fill=255)
    result = im.copy()
    result.putalpha(mask)
    return result

def main():
    print("Generating preview collage for YNAB Pulse...")
    
    images = []
    for filename, caption in SHOTS:
        path = os.path.join(SCREENSHOTS_DIR, filename)
        if not os.path.isfile(path):
            print(f"Warning: missing screenshot {filename}, creating placeholder...")
            im = Image.new("RGB", (720, 960), (30, 32, 40))
        else:
            im = Image.open(path).convert("RGBA")
        images.append((im, caption))

    cols = 3
    rows = 2
    
    target_w = 672
    target_h = 928
    
    gap_x = 44
    gap_y = 50
    padding_x = 56
    padding_top = 170
    padding_bottom = 56
    
    total_w = padding_x * 2 + cols * target_w + (cols - 1) * gap_x
    total_h = padding_top + rows * target_h + (rows - 1) * gap_y + padding_bottom
    
    # Modern deep dark backdrop (#0d1117)
    canvas = Image.new("RGBA", (total_w, total_h), (13, 17, 23, 255))
    draw = ImageDraw.Draw(canvas)
    
    # Typography
    title_font = load_font(64, bold=True)
    sub_font = load_font(26, bold=False)
    badge_font = load_font(22, bold=True)
    
    # Draw Title Header
    title_text = "YNAB Pulse"
    sub_text = "Real-Time Budget & Spending Intelligence for Omarchy Shell"
    
    # Green accent indicator dot next to title
    draw.text((total_w // 2, 70), title_text, fill=(240, 246, 252, 255), font=title_font, anchor="mm")
    draw.text((total_w // 2, 128), sub_text, fill=(139, 148, 158, 255), font=sub_font, anchor="mm")
    
    # Place screenshots
    for idx, (im, caption) in enumerate(images):
        c = idx % cols
        r = idx // cols
        
        x = padding_x + c * (target_w + gap_x)
        y = padding_top + r * (target_h + gap_y)
        
        im_resized = im.resize((target_w, target_h), Image.Resampling.LANCZOS)
        
        # Soft card shadow
        shadow_box = Image.new("RGBA", (target_w + 32, target_h + 32), (0, 0, 0, 0))
        sdraw = ImageDraw.Draw(shadow_box)
        sdraw.rounded_rectangle([(16, 16), (target_w + 16, target_h + 16)], radius=18, fill=(0, 0, 0, 160))
        shadow_blurred = shadow_box.filter(ImageFilter.GaussianBlur(12))
        canvas.paste(shadow_blurred, (x - 16, y - 12), shadow_blurred)
        
        # Rounded corners & border
        im_rounded = round_corners(im_resized, 14)
        canvas.paste(im_rounded, (x, y), im_rounded)
        
        # Outer card stroke
        draw.rounded_rectangle([(x, y), (x + target_w, y + target_h)], radius=14, outline=(48, 54, 61, 220), width=2)
        
        # Bottom Card Label Pill
        pill_w = len(caption) * 13 + 44
        pill_h = 38
        pill_x = x + (target_w - pill_w) // 2
        pill_y = y + target_h - pill_h - 18
        
        draw.rounded_rectangle([(pill_x, pill_y), (pill_x + pill_w, pill_y + pill_h)], radius=12, fill=(18, 22, 29, 240), outline=(56, 139, 253, 180), width=1)
        draw.text((pill_x + pill_w // 2, pill_y + pill_h // 2), caption, fill=(240, 246, 252, 255), font=badge_font, anchor="mm")

    # Save output
    rgb_canvas = canvas.convert("RGB")
    rgb_canvas.save(OUTPUT_PATH, quality=95)
    print(f"Collage saved successfully to {OUTPUT_PATH} ({total_w}x{total_h})")

if __name__ == "__main__":
    main()
