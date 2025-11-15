/*
  Generate a Windows .ico from src-tauri/icons/icon.png if missing.
  This avoids tauri-build failing on Windows.
*/
const fs = require('fs');
const path = require('path');

const pngToIco = require('png-to-ico');

const root = path.resolve(__dirname, '..');
const iconsDir = path.join(root, 'src-tauri', 'icons');
const pngPath = path.join(iconsDir, 'icon.png');
const icoPath = path.join(iconsDir, 'icon.ico');

(async () => {
  try {
    if (!fs.existsSync(pngPath)) {
      console.error(`[gen-ico] PNG not found at ${pngPath}`);
      process.exit(0);
    }
    if (fs.existsSync(icoPath)) {
      console.log('[gen-ico] icon.ico already exists; skipping');
      process.exit(0);
    }

    console.log('[gen-ico] Generating icon.ico from icon.png...');
    const buf = await pngToIco(pngPath);
    fs.writeFileSync(icoPath, buf);
    console.log('[gen-ico] Wrote', icoPath);
  } catch (err) {
    console.error('[gen-ico] Failed to generate icon.ico:', err.message);
    // Do not fail the build; just warn. Tauri will still require the file.
    process.exit(0);
  }
})();
