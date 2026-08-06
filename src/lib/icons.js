// Phosphor icons from @stnd/icon, inlined at build time via vite `?raw`.
// The Astro apps fetch /icons/{prefix}/{name}.svg at runtime (copied by the
// @stnd/core integration); a Tauri static bundle has no such copy step yet,
// so the app imports exactly the icons it uses and ships them in the JS.
import arrowsClockwise from "../../../../packages/icon/icons/ph/arrows-clockwise.svg?raw";
import arrowsDownUp from "../../../../packages/icon/icons/ph/arrows-down-up.svg?raw";
import arrowsInSimple from "../../../../packages/icon/icons/ph/arrows-in-simple.svg?raw";
import caretDown from "../../../../packages/icon/icons/ph/caret-down.svg?raw";
import caretRight from "../../../../packages/icon/icons/ph/caret-right.svg?raw";
import check from "../../../../packages/icon/icons/ph/check.svg?raw";
import checkCircle from "../../../../packages/icon/icons/ph/check-circle.svg?raw";
import copy from "../../../../packages/icon/icons/ph/copy.svg?raw";
import circleHalf from "../../../../packages/icon/icons/ph/circle-half.svg?raw";
import downloadSimple from "../../../../packages/icon/icons/ph/download-simple.svg?raw";
import eyeSlash from "../../../../packages/icon/icons/ph/eye-slash.svg?raw";
import folderOpen from "../../../../packages/icon/icons/ph/folder-open.svg?raw";
import folderPlus from "../../../../packages/icon/icons/ph/folder-plus.svg?raw";
import gear from "../../../../packages/icon/icons/ph/gear.svg?raw";
import gridFour from "../../../../packages/icon/icons/ph/grid-four.svg?raw";
import image from "../../../../packages/icon/icons/ph/image.svg?raw";
import imageBroken from "../../../../packages/icon/icons/ph/image-broken.svg?raw";
import lightning from "../../../../packages/icon/icons/ph/lightning.svg?raw";
import notePencil from "../../../../packages/icon/icons/ph/note-pencil.svg?raw";
import plus from "../../../../packages/icon/icons/ph/plus.svg?raw";
import rows from "../../../../packages/icon/icons/ph/rows.svg?raw";
import sidebarSimple from "../../../../packages/icon/icons/ph/sidebar-simple.svg?raw";
import stackSimple from "../../../../packages/icon/icons/ph/stack-simple.svg?raw";
import star from "../../../../packages/icon/icons/ph/star.svg?raw";
import stopCircle from "../../../../packages/icon/icons/ph/stop-circle.svg?raw";
import userCircle from "../../../../packages/icon/icons/ph/user-circle.svg?raw";
import warning from "../../../../packages/icon/icons/ph/warning.svg?raw";
import x from "../../../../packages/icon/icons/ph/x.svg?raw";

export const icons = {
  "arrows-clockwise": arrowsClockwise,
  "arrows-down-up": arrowsDownUp,
  "arrows-in-simple": arrowsInSimple,
  "caret-down": caretDown,
  "caret-right": caretRight,
  check,
  "check-circle": checkCircle,
  copy,
  "circle-half": circleHalf,
  "download-simple": downloadSimple,
  "eye-slash": eyeSlash,
  "folder-open": folderOpen,
  "folder-plus": folderPlus,
  gear,
  "grid-four": gridFour,
  image,
  "image-broken": imageBroken,
  lightning,
  "note-pencil": notePencil,
  plus,
  rows,
  "sidebar-simple": sidebarSimple,
  "stack-simple": stackSimple,
  star,
  "stop-circle": stopCircle,
  "user-circle": userCircle,
  warning,
  x,
};
