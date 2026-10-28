/**
 * The Malayalam fixity check messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Malayalam. Correct
 * a string here; a later run keeps it. It is typed `FixityMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { FixityMessages } from '../index';

export const ml: FixityMessages = {
  'fixity.class': 'സംഭരിച്ച ഫയലുകൾ പരിശോധിക്കുക',
  'fixity.start': 'സംഭരിച്ച ഫയലുകൾ പരിശോധിക്കുക',
  'fixity.start.note':
    'ഓരോ സംഭരിച്ച യഥാർത്ഥ ഫയലും വായിക്കുന്നു, ഫയൽ ചേർക്കുമ്പോൾ രേഖപ്പെടുത്തിയ SHA-256-മായി താരതമ്യം ചെയ്യുന്നു.',
  'fixity.start.busy': 'പരിശോധന ആരംഭിക്കുന്നു…',
  'fixity.start.failed': 'പരിശോധന ആരംഭിച്ചില്ല: {message}',
  'fixity.result.title': 'ഫലം',
  'fixity.result.loading': 'ഫലം ലോഡ് ചെയ്യുന്നു…',
  'fixity.result.failed': 'ഫലം വായിക്കാൻ കഴിഞ്ഞില്ല: {message}',
  'fixity.result.waiting': 'പരിശോധന അവസാനിക്കുമ്പോൾ ഫലം ഇവിടെ കാണിക്കും.',
  'fixity.result.none':
    'ഈ പരിശോധന ഒരു ഫലം എഴുതുന്നതിന് മുമ്പ് അവസാനിച്ചു. അതിന്റെ ലോഗ് കാരണം എന്തെന്ന് പറയുന്നു.',
  'fixity.result.checkedOne': '{date}-ൽ 1 ഫയൽ പരിശോധിച്ചു.',
  'fixity.result.checkedMany': '{count} ഫയലുകൾ {date}-ൽ പരിശോധിച്ചു.',
  'fixity.result.differsOne': '1 വ്യത്യാസമുണ്ട്.',
  'fixity.result.differsMany': '{count} വ്യത്യാസമുണ്ട്.',
  'fixity.result.missingOne': '1 കാണാനില്ല.',
  'fixity.result.missingMany': '{count} കാണാനില്ല.',
  'fixity.result.unreadableOne': '1 വായിക്കാൻ കഴിയില്ല.',
  'fixity.result.unreadableMany': '{count} വായിക്കാൻ കഴിയില്ല.',
  'fixity.result.allMatch': 'എല്ലാം പൊരുത്തപ്പെടുന്നു.',
  'fixity.result.stopped': 'അവസാന ഫയൽ എത്തുന്നതിന് മുമ്പ് പരിശോധന നിർത്തി.',
  'fixity.line.problemsOne': 'അവസാനമായി പരിശോധിച്ച {date}: 1 പ്രശ്നം',
  'fixity.line.problemsMany': 'അവസാനമായി പരിശോധിച്ച {date}: {count} പ്രശ്നങ്ങൾ',
  'fixity.line.noProblems': 'അവസാനമായി പരിശോധിച്ച {date}: പ്രശ്നങ്ങളില്ല',
  'fixity.line.never': 'സൂക്ഷിച്ച ഫയലുകൾ ഒരിക്കലും പരിശോധിച്ചിട്ടില്ല.',
  'fixity.line.running': 'സൂക്ഷിച്ച ഫയലുകൾ ഇപ്പോൾ പരിശോധിക്കുന്നു.',
  'fixity.filter.checked': '#{job} പരിശോധനയിൽ വായിച്ച ഫയലുകൾ.',
  'fixity.filter.ok': '#{job} പരിശോധനയിൽ പൊരുത്തപ്പെട്ട ഫയലുകൾ.',
  'fixity.filter.differs': '#{job} പരിശോധനയിൽ വ്യത്യാസമുള്ള ഫയലുകൾ.',
  'fixity.filter.missing': '#{job} പരിശോധനയിൽ ഇല്ലാത്ത ഫയലുകൾ.',
  'fixity.filter.unreadable': '#{job} പരിശോധനയിൽ വായിക്കാൻ കഴിയാത്ത ഫയലുകൾ.',
  'fixity.filter.back': 'പരിശോധന തുറക്കുക',
  'fixity.filter.clear': 'എല്ലാ മീഡിയ കാണിക്കുക',
};
