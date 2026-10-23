/**
 * The Hindi words of the Structure screen's corrections.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, then read against English and corrected by hand, and not yet
 * reviewed by a reader of Hindi. Correct a string here; a later run keeps it. It is
 * typed `ImportFixesMessages`, so a key added to English and not here is a `tsc` error.
 */
import type { ImportFixesMessages } from "../index";

export const hi: ImportFixesMessages = {
  "fix.q.items": "ऑफर: आइटम",
  "fix.q.rows": "ऑफर: पंक्तियाँ",
  "fix.oneItem": "{count} वस्तुएं किसी दूसरी वस्तु में जा सकती हैं।",
  "fix.oneItemOne": "1 वस्तु किसी दूसरी वस्तु में जा सकती है।",
  "fix.rowIs": "{count} पंक्तियाँ कोई फ़ाइल नाम नहीं देतीं।",
  "fix.rowIsOne": "1 पंक्ति कोई फ़ाइल नाम नहीं देती।",
  "fix.rowIs.note": "हर पंक्ति ऐसी एक फ़ाइल की ओर इशारा करती है जिसकी कोई पंक्ति नहीं है।",
  "fix.rowIs.noteOne": "यह ऐसी एक फ़ाइल की ओर इशारा करती है जिसकी कोई पंक्ति नहीं है।",
  "fix.describesNothing": "{count} पंक्तियाँ कोई फ़ाइल नाम नहीं देतीं, और कोई फ़ाइल मिलती-जुलती नहीं है।",
  "fix.describesNothingOne": "1 पंक्ति कोई फ़ाइल नाम नहीं देती, और कोई फ़ाइल मिलती-जुलती नहीं है।",
  "fix.describesNothing.note": "हर पंक्ति बिना फ़ाइल की एक वस्तु बनाती है।",
  "fix.describesNothing.noteOne": "यह बिना फ़ाइल की एक वस्तु बनाती है।",
  "fix.items": "वस्तुएं: {before} → {after}",
  "fix.rows": "फ़ाइल ढूँढने वाली पंक्तियाँ: {of} में से {before} → {after}",
  "fix.show": "{count} दिखाएँ",
  "fix.showOne": "इसे दिखाएँ",
  "fix.oneItem.take": "इन {count} को उनकी वस्तु में रखें",
  "fix.oneItem.takeOne": "इसे इसकी वस्तु में रखें",
  "fix.rowIs.take": "इन {count} को पेयर करें",
  "fix.rowIs.takeOne": "इस पंक्ति को पेयर करें",
  "fix.describesNothing.take": "इन {count} को छोड़ें",
  "fix.describesNothing.takeOne": "इस पंक्ति को छोड़ें",
  "fix.join": "{with}, {item} की वस्तु में जाती है",
  "fix.pair": "पंक्ति {line}, {cell}: फ़ाइल {file}",
  "fix.leave": "पंक्ति {line}, {cell}",
  "fix.because.keyExtends": "इसका नाम दूसरी वस्तु के नाम से शुरू होता है।",
  "fix.because.rowNamesBoth": "एक पंक्ति दोनों का नाम देती है।",
  "fix.because.near": "नामों में एक अक्षर का अंतर है।",
  "fix.because.namesItem": "पंक्ति इस फ़ाइल की वस्तु का नाम देती है।",
  "fix.because.namesPart": "पंक्ति अपनी वस्तु में इस फ़ाइल के हिस्से का नाम देती है।",
  "fix.joined": "दूसरी वस्तु में: {count}",
  "fix.paired": "पेयर की गईं: {count}",
  "fix.left": "छोड़ी गईं: {count}",
  "fix.stale": "{count} अब लागू नहीं होते।",
  "fix.staleOne": "1 अब लागू नहीं होता।",
  "fix.rowChanged": "यह पंक्ति बदल गई।",
  "fix.fileGone": "यह फ़ाइल अब नहीं है।",
};
