# This script requires inkscape installed to generate the various sized icons for this app, we need the following
# icon list (and location):
#    File: beacn-utility.svg, Size: 1024x1024 Target: /usr/share/icons/hicolor/scalable/apps/beacn-utility.svg
#    File: beacn-utility.png, Size: 48x48, Target: /usr/share/icons/hicolor/48x48/apps/beacn-utility.png
#    File: beacn-utility-large.png, Size: 128x128, Target: /usr/share/pixmaps/beacn-utility.png

# Generate the Files
#inkscape beacn-utility-old.svg --export-filename=beacn-utility-large.png -w 128 -h 128 --export-area=-100:-100:1124:1124
#inkscape beacn-utility-old.svg --export-filename=beacn-utility.png -w 48 -h 48 --export-area=-100:-100:1124:1124

inkscape beacn-utility.svg --export-filename=beacn-utility-large.png -w 128 -h 128 --export-area=-100:-100:1124:1124
inkscape beacn-utility.svg --export-filename=beacn-utility.png -w 48 -h 48 --export-area=-100:-100:1124:1124