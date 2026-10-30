/**
 * The Kannada item history messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Kannada. Correct
 * a string here; a later run keeps it. It is typed `ItemHistoryMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { ItemHistoryMessages } from '../index';

export const kn: ItemHistoryMessages = {
  'itemHistory.loading': 'ಇತಿಹಾಸವನ್ನು ಓದುತ್ತಿದೆ…',
  'itemHistory.empty': 'ಈ ಐಟಂಗೆ ಇನ್ನೂ ಯಾವುದೇ ಬದಲಾವಣೆಯನ್ನು ದಾಖಲಿಸಲಾಗಿಲ್ಲ.',
  'itemHistory.entry': '{date} · {who}',
  'itemHistory.entrySource': '{date} · {who} · {source}',
  'itemHistory.deletedUser': 'ಅಳಿಸಲಾದ ಬಳಕೆದಾರ',
  'itemHistory.source.bulk': 'ಬಲ್ಕ್ ಬದಲಾವಣೆ {id}',
  'itemHistory.source.import': 'ಆಮದು {id}',
  'itemHistory.source.bulkUndo': 'ಬಲ್ಕ್ ಬದಲಾವಣೆಯ {id} ರದ್ದು',
  'itemHistory.source.importUndo': 'ಆಮದು {id} ರದ್ದು',
  'itemHistory.change': '{field}: {before} → {after}',
  'itemHistory.field.visibility': 'ಗೋಚರತೆ',
  'itemHistory.field.class': 'ವರ್ಗ',
  'itemHistory.field.template': 'ಐಟಂ ಪ್ರಕಾರ',
  'itemHistory.field.itemSets': 'ಐಟಂ ಸೆಟ್‌ಗಳು',
  'itemHistory.field.sites': 'ಸೈಟ್‌ಗಳು',
  'itemHistory.public': 'ಸಾರ್ವಜನಿಕ',
  'itemHistory.private': 'ಖಾಸಗಿ',
  'itemHistory.none': '(ಯಾವುದೂ ಇಲ್ಲ)',
  'itemHistory.inTrash': '{name} (ಟ್ರಾಶ್‌ನಲ್ಲಿ)',
  'itemHistory.deletedSet': 'ಅಳಿಸಲಾದ ಐಟಂ ಸೆಟ್ {id}',
  'itemHistory.privateValue': '{value} (ಖಾಸಗಿ)',
  'itemHistory.putBack': 'ಈ ಮೌಲ್ಯವನ್ನು ಹಿಂತಿರುಗಿಸಿ',
  'itemHistory.puttingBack': 'ಮೌಲ್ಯವನ್ನು ಹಿಂತಿರುಗಿಸಲಾಗುತ್ತಿದೆ…',
  'itemHistory.putBackDone': 'ಮೌಲ್ಯವು ಮತ್ತೆ ಬಂದಿದೆ. ಮೇಲಿನ ಹೊಸ ಬದಲಾವಣೆಯು ಇದನ್ನು ತಿಳಿಸುತ್ತದೆ.',
  'itemHistory.putBackSame': 'ಏನೂ ಬದಲಾಗಿಲ್ಲ: ಐಟಂ ಈಗ ಈ ಮೌಲ್ಯವನ್ನು ಹೊಂದಿದೆ.',
  'itemHistory.older': 'ಹಳೆಯ ಬದಲಾವಣೆಗಳನ್ನು ತೋರಿಸಿ',
};
