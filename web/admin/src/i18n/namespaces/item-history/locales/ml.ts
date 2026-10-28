/**
 * The Malayalam item history messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Malayalam. Correct
 * a string here; a later run keeps it. It is typed `ItemHistoryMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ItemHistoryMessages } from '../index';

export const ml: ItemHistoryMessages = {
  'itemHistory.loading': 'ചരിത്രം വായിക്കുന്നു…',
  'itemHistory.empty': 'ഈ ഇനത്തിൽ ഇതുവരെ മാറ്റങ്ങൾ രേഖപ്പെടുത്തിയിട്ടില്ല.',
  'itemHistory.entry': '{date} · {who}',
  'itemHistory.entrySource': '{date} · {who} · {source}',
  'itemHistory.deletedUser': 'നീക്കം ചെയ്ത ഉപയോക്താവ്',
  'itemHistory.source.bulk': 'ബൾക്ക് മാറ്റം {id}',
  'itemHistory.source.import': 'ഇംപോർട്ട് {id}',
  'itemHistory.source.bulkUndo': 'ബൾക്ക് മാറ്റത്തിന്റെ അൺഡൂ {id}',
  'itemHistory.source.importUndo': 'ഇംപോർട്ടിന്റെ അൺഡൂ {id}',
  'itemHistory.change': '{field}: {before} → {after}',
  'itemHistory.field.visibility': 'ദൃശ്യപരത',
  'itemHistory.field.class': 'ക്ലാസ്',
  'itemHistory.field.template': 'ഇനം തരം',
  'itemHistory.field.itemSets': 'ഇനം സെറ്റുകൾ',
  'itemHistory.field.sites': 'സൈറ്റുകൾ',
  'itemHistory.public': 'പബ്ലിക്',
  'itemHistory.private': 'പ്രൈവറ്റ്',
  'itemHistory.none': '(ഒന്നുമില്ല)',
  'itemHistory.untitled': 'പേര് നൽകാത്ത {id}',
  'itemHistory.inTrash': '{name} (ട്രാഷിൽ)',
  'itemHistory.deletedSet': 'ഡിലീറ്റ് ചെയ്ത ഇനം സെറ്റ് {id}',
  'itemHistory.privateValue': '{value} (പ്രൈവറ്റ്)',
  'itemHistory.putBack': 'ഈ മൂല്യം തിരികെ നൽകുക',
  'itemHistory.puttingBack': 'മൂല്യം തിരികെ വെക്കുന്നു…',
  'itemHistory.putBackDone':
    'മൂല്യം തിരികെ ലഭിച്ചു. ഏറ്റവും പുതിയ മാറ്റം മുകളിൽ അത് വ്യക്തമാക്കുന്നു.',
  'itemHistory.putBackSame': 'മാറ്റമില്ല: ഇനത്തിന് ഇപ്പോൾ ഈ മൂല്യമുണ്ട്.',
  'itemHistory.older': 'പഴയ മാറ്റങ്ങൾ കാണിക്കുക',
};
