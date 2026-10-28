/**
 * The Kannada fixity check messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Kannada. Correct
 * a string here; a later run keeps it. It is typed `FixityMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { FixityMessages } from '../index';

export const kn: FixityMessages = {
  'fixity.class': 'ಸಂಗ್ರಹಿಸಿದ ಫೈಲ್‌ಗಳನ್ನು ಪರಿಶೀಲಿಸಿ',
  'fixity.start': 'ಸಂಗ್ರಹಿಸಿದ ಫೈಲ್‌ಗಳನ್ನು ಪರಿಶೀಲಿಸಿ',
  'fixity.start.note':
    'ಇದು ಸಂಗ್ರಹಿಸಿದ ಪ್ರತಿಯೊಂದು ಮೂಲವನ್ನು ಓದುತ್ತದೆ ಮತ್ತು ಫೈಲ್ ಅನ್ನು ಸೇರಿಸಿದಾಗ ದಾಖಲಾದ SHA-256 ನೊಂದಿಗೆ ಅದನ್ನು ಹೋಲಿಸುತ್ತದೆ.',
  'fixity.start.busy': 'ಪರಿಶೀಲನೆ ಪ್ರಾರಂಭಿಸಲಾಗುತ್ತಿದೆ…',
  'fixity.start.failed': 'ಪರಿಶೀಲನೆಯು ಪ್ರಾರಂಭವಾಗಲಿಲ್ಲ: {message}',
  'fixity.result.title': 'ಫಲಿತಾಂಶ',
  'fixity.result.loading': 'ಫಲಿತಾಂಶವನ್ನು ಲೋಡ್ ಮಾಡಲಾಗುತ್ತಿದೆ…',
  'fixity.result.failed': 'ಫಲಿತಾಂಶವನ್ನು ಓದಲು ಸಾಧ್ಯವಾಗಲಿಲ್ಲ: {message}',
  'fixity.result.waiting': 'ಪರಿಶೀಲನೆ ಮುಗಿದಾಗ ಇಲ್ಲಿ ಫಲಿತಾಂಶವನ್ನು ತೋರಿಸಲಾಗುತ್ತದೆ.',
  'fixity.result.none':
    'ಈ ಪರಿಶೀಲನೆಯು ಫಲಿತಾಂಶವನ್ನು ಬರೆಯುವ ಮೊದಲು ಕೊನೆಗೊಂಡಿತು. ಇದರ ಲಾಗ್ ಅದು ಏಕೆ ಎಂದು ಹೇಳುತ್ತದೆ.',
  'fixity.result.checkedOne': '{date} ರಂದು 1 ಫೈಲ್ ಅನ್ನು ಪರಿಶೀಲಿಸಲಾಗಿದೆ.',
  'fixity.result.checkedMany': '{count} ಫೈಲ್‌ಗಳನ್ನು {date} ರಂದು ಪರಿಶೀಲಿಸಲಾಗಿದೆ.',
  'fixity.result.differsOne': '1 ವ್ಯತ್ಯಾಸವಿದೆ.',
  'fixity.result.differsMany': '{count} ವ್ಯತ್ಯಾಸವಿದೆ.',
  'fixity.result.missingOne': '1 ಕಾಣೆಯಾಗಿದೆ.',
  'fixity.result.missingMany': '{count} ಕಾಣೆಯಾಗಿದೆ.',
  'fixity.result.unreadableOne': '1 ಓದಲು ಸಾಧ್ಯವಾಗುತ್ತಿಲ್ಲ.',
  'fixity.result.unreadableMany': '{count} ಓದಲು ಸಾಧ್ಯವಾಗುತ್ತಿಲ್ಲ.',
  'fixity.result.allMatch': 'ಎಲ್ಲವೂ ಹೊಂದಿಕೆಯಾಗುತ್ತವೆ.',
  'fixity.result.stopped': 'ಕೊನೆಯ ಫೈಲ್ ಬರುವ ಮೊದಲೇ ಪರಿಶೀಲನೆಯನ್ನು ನಿಲ್ಲಿಸಲಾಯಿತು.',
  'fixity.line.problemsOne': 'ಕೊನೆಯದಾಗಿ ಪರಿಶೀಲಿಸಿದ {date}: 1 ಸಮಸ್ಯೆ',
  'fixity.line.problemsMany': 'ಕೊನೆಯದಾಗಿ ಪರಿಶೀಲಿಸಿದ {date}: {count} ಸಮಸ್ಯೆಗಳು',
  'fixity.line.noProblems': 'ಕೊನೆಯದಾಗಿ ಪರಿಶೀಲಿಸಿದ {date}: ಯಾವುದೇ ಸಮಸ್ಯೆಗಳಿಲ್ಲ',
  'fixity.line.never': 'ಸಂಗ್ರಹಿಸಿದ ಫೈಲ್‌ಗಳನ್ನು ಎಂದಿಗೂ ಪರಿಶೀಲಿಸಲಾಗಿಲ್ಲ.',
  'fixity.line.running': 'ಸಂಗ್ರಹಿಸಿದ ಫೈಲ್‌ಗಳನ್ನು ಈಗ ಪರಿಶೀಲಿಸಲಾಗುತ್ತಿದೆ.',
  'fixity.filter.checked': '#{job} ಪರಿಶೀಲನೆಯಲ್ಲಿ ಓದಿದ ಫೈಲ್‌ಗಳು.',
  'fixity.filter.ok': '#{job} ಪರಿಶೀಲನೆಯಲ್ಲಿ ಹೊಂದಿಕೆಯಾದ ಫೈಲ್‌ಗಳು.',
  'fixity.filter.differs': '#{job} ಪರಿಶೀಲನೆಯಲ್ಲಿ ವ್ಯತ್ಯಾಸವಿರುವ ಫೈಲ್‌ಗಳು.',
  'fixity.filter.missing': '#{job} ಪರಿಶೀಲನೆಯಲ್ಲಿ ಇಲ್ಲದ ಫೈಲ್‌ಗಳು.',
  'fixity.filter.unreadable': '#{job} ಪರಿಶೀಲನೆಯಲ್ಲಿ ಓದಲು ಸಾಧ್ಯವಾಗದ ಫೈಲ್‌ಗಳು.',
  'fixity.filter.back': 'ಪರಿಶೀಲನೆಯನ್ನು ತೆರೆಯಿರಿ',
  'fixity.filter.clear': 'ಎಲ್ಲಾ ಮಾಧ್ಯಮಗಳನ್ನು ತೋರಿಸಿ',
};
