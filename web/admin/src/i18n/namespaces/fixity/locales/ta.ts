/**
 * The Tamil fixity check messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Tamil. Correct
 * a string here; a later run keeps it. It is typed `FixityMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { FixityMessages } from '../index';

export const ta: FixityMessages = {
  'fixity.class': 'சேமிக்கப்பட்ட கோப்புகளைச் சரிபார்க்கவும்',
  'fixity.start': 'சேமிக்கப்பட்ட கோப்புகளைச் சரிபார்க்கவும்',
  'fixity.start.note':
    'ஒவ்வொரு சேமிக்கப்பட்ட அசல் கோப்பையும் படித்து, கோப்பு சேர்க்கப்பட்டபோது பதிவு செய்யப்பட்ட SHA-256 உடன் ஒப்பிடுகிறது.',
  'fixity.start.busy': 'சரிபார்ப்பு தொடங்குகிறது…',
  'fixity.start.failed': 'சரிபார்ப்பு தொடங்கவில்லை: {message}',
  'fixity.result.title': 'முடிவு',
  'fixity.result.loading': 'முடிவை ஏற்றுகிறது…',
  'fixity.result.failed': 'முடிவை வாசிக்க முடியவில்லை: {message}',
  'fixity.result.waiting': 'சரிபார்ப்பு முடிந்ததும் முடிவு இங்கே காட்டப்படும்.',
  'fixity.result.none':
    'இந்தச் சரிபார்ப்பு ஒரு முடிவை எழுதாமல் முடிந்தது. அதன் பதிவேடு அது ஏன் என்று விளக்குகிறது.',
  'fixity.result.checkedOne': '{date} அன்று 1 கோப்பு சரிபார்க்கப்பட்டது.',
  'fixity.result.checkedMany': '{count} கோப்புகள் {date} அன்று சரிபார்க்கப்பட்டன.',
  'fixity.result.differsOne': '1 வேறுபடுகிறது.',
  'fixity.result.differsMany': '{count} வேறுபடுகின்றன.',
  'fixity.result.missingOne': '1 விடுபட்டுள்ளது.',
  'fixity.result.missingMany': '{count} விடுபட்டுள்ளன.',
  'fixity.result.unreadableOne': '1 வாசிக்க முடியவில்லை.',
  'fixity.result.unreadableMany': '{count} வாசிக்க முடியவில்லை.',
  'fixity.result.allMatch': 'அனைத்தும் பொருந்துகின்றன.',
  'fixity.result.stopped': 'கடைசி கோப்பிற்கு முன்பே சரிபார்ப்பு நிறுத்தப்பட்டது.',
  'fixity.line.problemsOne': 'கடைசியாகச் சரிபார்க்கப்பட்ட கோப்புகள் {date}: 1 சிக்கல்',
  'fixity.line.problemsMany': 'கடைசியாகச் சரிபார்க்கப்பட்ட கோப்புகள் {date}: {count} சிக்கல்கள்',
  'fixity.line.noProblems': 'கடைசியாகச் சரிபார்க்கப்பட்ட கோப்புகள் {date}: எந்தச் சிக்கலும் இல்லை',
  'fixity.line.never': 'சேமிக்கப்பட்ட கோப்புகள் ஒருபோதும் சரிபார்க்கப்படவில்லை.',
  'fixity.line.running': 'சேமிக்கப்பட்ட கோப்புகள் இப்போது சரிபார்க்கப்படுகின்றன.',
  'fixity.filter.checked': '#{job} சரிபார்ப்பால் வாசிக்கப்பட்ட கோப்புகள்.',
  'fixity.filter.ok': '#{job} சரிபார்ப்பில் பொருந்திய கோப்புகள்.',
  'fixity.filter.differs': '#{job} சரிபார்ப்பில் வேறுபடும் கோப்புகள்.',
  'fixity.filter.missing': '#{job} சரிபார்ப்பில் விடுபட்ட கோப்புகள்.',
  'fixity.filter.unreadable': '#{job} சரிபார்ப்பில் வாசிக்க முடியாத கோப்புகள்.',
  'fixity.filter.back': 'சரிபார்ப்பைத் திறக்கவும்',
  'fixity.filter.clear': 'அனைத்து ஊடகங்களையும் காட்டவும்',
};
