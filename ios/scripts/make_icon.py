"""Draws the app icon (ios/XPRouter/Assets.xcassets/AppIcon.appiconset/icon.png):
a warm sunset gradient with a rising route line through checkpoint dots and
a level-up chevron. Run: python3 ios/scripts/make_icon.py"""
import os
from PIL import Image, ImageDraw, ImageFilter, ImageFont

S = 1024
here = os.path.dirname(os.path.abspath(__file__))
out = os.path.join(here, "..", "XPRouter", "Assets.xcassets", "AppIcon.appiconset", "icon.png")

def lerp(a, b, t):
    return tuple(int(a[i] + (b[i] - a[i]) * t) for i in range(3))

img = Image.new("RGB", (S, S))
top, bottom = (255, 128, 72), (196, 40, 92)
px = img.load()
for y in range(S):
    for x in range(S):
        t = (y * 0.75 + x * 0.25) / S
        px[x, y] = lerp(top, bottom, min(1, t))

# soft glow
glow = Image.new("RGBA", (S, S), (0, 0, 0, 0))
g = ImageDraw.Draw(glow)
g.ellipse((560, -180, 1240, 500), fill=(255, 220, 140, 120))
glow = glow.filter(ImageFilter.GaussianBlur(120))
img.paste(glow, (0, 0), glow)

d = ImageDraw.Draw(img, "RGBA")
# grid of faint route steps
for i in range(1, 8):
    y = S - i * 128
    d.line((80, y, S - 80, y), fill=(255, 255, 255, 22), width=4)

pts = [(150, 820), (330, 700), (470, 735), (640, 520), (790, 430)]
shadow = [(x + 10, y + 16) for x, y in pts]
d.line(shadow, fill=(90, 10, 40, 90), width=58, joint="curve")
d.line(pts, fill=(255, 255, 255, 255), width=52, joint="curve")
for (x, y) in pts[:-1]:
    r = 44
    d.ellipse((x - r, y - r, x + r, y + r), fill=(255, 255, 255, 255))
    r2 = 22
    d.ellipse((x - r2, y - r2, x + r2, y + r2), fill=(214, 64, 96, 255))
# arrow head at the end
x, y = pts[-1]
d.polygon([(x + 120, y - 95), (x - 40, y - 70), (x + 70, y + 60)], fill=(255, 255, 255, 255))

try:
    font = ImageFont.truetype("/System/Library/Fonts/SFCompactRounded.ttf", 250)
    try:
        font.set_variation_by_name("Heavy")
    except Exception:
        font.set_variation_by_axes([900])
except Exception:
    font = ImageFont.load_default()
d.text((120, 130), "XP", font=font, fill=(255, 255, 255, 240))
img.save(out)
print("wrote", out)
