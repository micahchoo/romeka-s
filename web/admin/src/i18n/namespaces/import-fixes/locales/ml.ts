/**
 * The Malayalam words of the Structure screen's corrections.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, then read against English and corrected by hand, and not yet
 * reviewed by a reader of Malayalam. Correct a string here; a later run keeps it. It is
 * typed `ImportFixesMessages`, so a key added to English and not here is a `tsc` error.
 */
import type { ImportFixesMessages } from "../index";

export const ml: ImportFixesMessages = {
  'fix.q.items': 'ഓഫർ: ഇനങ്ങൾ',
  'fix.q.rows': 'ഓഫർ: വരികൾ',
  'fix.oneItem': '{count} ഇനങ്ങൾക്ക് മറ്റൊരു ഇനത്തിനുള്ളിൽ പോകാം.',
  'fix.oneItemOne': '1 ഇനത്തിന് മറ്റൊരു ഇനത്തിനുള്ളിൽ പോകാം.',
  'fix.rowIs': '{count} വരികൾ ഒരു ഫയലിന്റെയും പേര് നൽകുന്നില്ല.',
  'fix.rowIsOne': '1 വരി ഒരു ഫയലിന്റെയും പേര് നൽകുന്നില്ല.',
  'fix.rowIs.note': 'ഓരോ വരിയും വരിയില്ലാത്ത ഒരു ഫയലിനെ സൂചിപ്പിക്കുന്നു.',
  'fix.rowIs.noteOne': 'അത് വരിയില്ലാത്ത ഒരു ഫയലിനെ സൂചിപ്പിക്കുന്നു.',
  'fix.describesNothing':
    '{count} വരികൾ ഒരു ഫയലിന്റെയും പേര് നൽകുന്നില്ല, പേരിൽ അടുത്ത ഫയലും ഇല്ല.',
  'fix.describesNothingOne': '1 വരി ഒരു ഫയലിന്റെയും പേര് നൽകുന്നില്ല, പേരിൽ അടുത്ത ഫയലും ഇല്ല.',
  'fix.describesNothing.note': 'ഓരോ വരിയും ഫയലില്ലാത്ത ഒരു ഇനം ഉണ്ടാക്കുന്നു.',
  'fix.describesNothing.noteOne': 'അത് ഫയലില്ലാത്ത ഒരു ഇനം ഉണ്ടാക്കുന്നു.',
  'fix.items': 'ഇനങ്ങൾ: {before} → {after}',
  'fix.rows': 'ഫയൽ കണ്ടെത്തുന്ന വരികൾ: {of} എണ്ണത്തിൽ {before} → {after}',
  'fix.show': '{count} എണ്ണം കാണിക്കുക',
  'fix.showOne': 'അത് കാണിക്കുക',
  'fix.oneItem.take': 'ഈ {count} എണ്ണം അവയുടെ ഇനത്തിൽ ചേർക്കുക',
  'fix.oneItem.takeOne': 'അതിനെ അതിന്റെ ഇനത്തിൽ ചേർക്കുക',
  'fix.rowIs.take': 'ഈ {count} എണ്ണം ജോടിയാക്കുക',
  'fix.rowIs.takeOne': 'ഈ വരി ജോടിയാക്കുക',
  'fix.describesNothing.take': 'ഈ {count} എണ്ണം ഒഴിവാക്കുക',
  'fix.describesNothing.takeOne': 'ഈ വരി ഒഴിവാക്കുക',
  'fix.join': '{with}, {item}-ന്റെ ഇനത്തിലേക്ക് പോകുന്നു',
  'fix.pair': 'വരി {line}, {cell}: ഫയൽ {file}',
  'fix.leave': 'വരി {line}, {cell}',
  'fix.because.keyExtends': 'ഇതിന്റെ പേര് മറ്റേ ഇനത്തിന്റെ പേരിൽ തുടങ്ങുന്നു.',
  'fix.because.rowNamesBoth': 'ഒരു വരി രണ്ടിന്റെയും പേര് നൽകുന്നു.',
  'fix.because.near': 'പേരുകൾ തമ്മിൽ ഒരു അക്ഷരത്തിന്റെ വ്യത്യാസമുണ്ട്.',
  'fix.because.namesItem': 'വരി ഈ ഫയലിന്റെ ഇനത്തിന്റെ പേര് നൽകുന്നു.',
  'fix.because.namesPart': 'വരി അതിന്റെ ഇനത്തിൽ ഈ ഫയലിന്റെ ഭാഗത്തിന്റെ പേര് നൽകുന്നു.',
  'fix.joined': 'മറ്റൊരു ഇനത്തിലേക്ക്: {count}',
  'fix.paired': 'ജോടിയാക്കിയവ: {count}',
  'fix.left': 'ഒഴിവാക്കിയവ: {count}',
  'fix.stale': '{count} എണ്ണം ഇനി ബാധകമല്ല.',
  'fix.staleOne': '1 എണ്ണം ഇനി ബാധകമല്ല.',
  'fix.rowChanged': 'ഈ വരി മാറി.',
  'fix.fileGone': 'ഈ ഫയൽ ഇപ്പോൾ ഇല്ല.',
};
