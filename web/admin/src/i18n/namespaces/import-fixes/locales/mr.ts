/**
 * The Marathi words of the Structure screen's corrections.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, then read against English and corrected by hand, and not yet
 * reviewed by a reader of Marathi. Correct a string here; a later run keeps it. It is
 * typed `ImportFixesMessages`, so a key added to English and not here is a `tsc` error.
 */
import type { ImportFixesMessages } from "../index";

export const mr: ImportFixesMessages = {
  "fix.q.items": "ऑफर: घटक",
  "fix.q.rows": "ऑफर: ओळी",
  "fix.oneItem": "{count} घटक दुसऱ्या घटकात जाऊ शकतात.",
  "fix.oneItemOne": "1 घटक दुसऱ्या घटकात जाऊ शकतो.",
  "fix.rowIs": "{count} ओळी एक फाईल नाव देत नाहीत.",
  "fix.rowIsOne": "1 ओळ एक फाईल नाव देत नाही.",
  "fix.rowIs.note": "प्रत्येक ओळ अशा एका फाईलकडे निर्देश करते जिला ओळ नाही.",
  "fix.rowIs.noteOne": "ती अशा एका फाईलकडे निर्देश करते जिला ओळ नाही.",
  "fix.describesNothing": "{count} ओळी एक फाईल नाव देत नाहीत, आणि कोणतीही फाईल नावाने जवळ नाही.",
  "fix.describesNothingOne": "1 ओळ एक फाईल नाव देत नाही, आणि कोणतीही फाईल नावाने जवळ नाही.",
  "fix.describesNothing.note": "प्रत्येक ओळ फाईलशिवाय एक घटक तयार करते.",
  "fix.describesNothing.noteOne": "ती फाईलशिवाय एक घटक तयार करते.",
  "fix.items": "घटक: {before} → {after}",
  "fix.rows": "फाईल सापडणाऱ्या ओळी: {of} पैकी {before} → {after}",
  "fix.show": "{count} दाखवा",
  "fix.showOne": "ती दाखवा",
  "fix.oneItem.take": "हे {count} त्यांच्या घटकात ठेवा",
  "fix.oneItem.takeOne": "हे त्याच्या घटकात ठेवा",
  "fix.rowIs.take": "या {count} ची जोडी लावा",
  "fix.rowIs.takeOne": "या ओळीची जोडी लावा",
  "fix.describesNothing.take": "या {count} वगळा",
  "fix.describesNothing.takeOne": "ही ओळ वगळा",
  "fix.join": "{with} हे {item} च्या घटकात जाते",
  "fix.pair": "ओळ {line}, {cell}: फाईल {file}",
  "fix.leave": "ओळ {line}, {cell}",
  "fix.because.keyExtends": "याचे नाव दुसऱ्या घटकाच्या नावाने सुरू होते.",
  "fix.because.rowNamesBoth": "एक ओळ दोघांचे नाव देते.",
  "fix.because.near": "नावांमध्ये एका अक्षराचा फरक आहे.",
  "fix.because.namesItem": "ओळ या फाईलच्या घटकाचे नाव देते.",
  "fix.because.namesPart": "ओळ तिच्या घटकातील या फाईलच्या भागाचे नाव देते.",
  "fix.joined": "दुसऱ्या घटकात: {count}",
  "fix.paired": "जोडी लावलेल्या: {count}",
  "fix.left": "वगळलेल्या: {count}",
  "fix.stale": "{count} आता लागू होत नाहीत.",
  "fix.staleOne": "1 आता लागू होत नाही.",
  "fix.rowChanged": "ही ओळ बदलली आहे.",
  "fix.fileGone": "ही फाईल आता नाही.",
};
