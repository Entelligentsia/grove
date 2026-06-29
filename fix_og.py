from PIL import Image

# Open the original image
img = Image.open('site/assets/og_banner.png')

# Resize to 1200x1200 using high quality Lanczos filter
img_resized = img.resize((1200, 1200), Image.Resampling.LANCZOS)

# Crop the center 1200x630
# left, upper, right, lower
# center is 600. upper = 600 - 315 = 285. lower = 600 + 315 = 915
img_cropped = img_resized.crop((0, 285, 1200, 915))

# Save as actual PNG
img_cropped.save('site/assets/og_banner_1200.png', format='PNG')
print("Image saved successfully.")
