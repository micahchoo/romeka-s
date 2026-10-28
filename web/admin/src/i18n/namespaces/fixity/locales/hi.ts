/**
 * The Hindi fixity check messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Hindi. Correct
 * a string here; a later run keeps it. It is typed `FixityMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { FixityMessages } from '../index';

export const hi: FixityMessages = {
  'fixity.class': 'संग्रहीत फ़ाइलों की जाँच करें',
  'fixity.start': 'संग्रहीत फ़ाइलों की जाँच करें',
  'fixity.start.note':
    'प्रत्येक संग्रहीत मूल फ़ाइल को पढ़ता है और इसकी तुलना उस SHA-256 से करता है जो फ़ाइल जोड़े जाने के समय दर्ज किया गया था।',
  'fixity.start.busy': 'जाँच शुरू हो रही है…',
  'fixity.start.failed': 'जाँच शुरू नहीं हुई: {message}',
  'fixity.result.title': 'परिणाम',
  'fixity.result.loading': 'परिणाम लोड हो रहा है…',
  'fixity.result.failed': 'परिणाम पढ़ा नहीं जा सका: {message}',
  'fixity.result.waiting': 'जाँच समाप्त होने पर परिणाम यहाँ प्रदर्शित होता है।',
  'fixity.result.none':
    'यह जाँच परिणाम लिखे जाने से पहले समाप्त हो गई। इसका लॉग बताता है कि क्यों।',
  'fixity.result.checkedOne': '{date} को 1 फ़ाइल की जाँच की गई।',
  'fixity.result.checkedMany': '{count} फ़ाइलें {date} को जाँची गईं।',
  'fixity.result.differsOne': '1 भिन्न है।',
  'fixity.result.differsMany': '{count} भिन्न हैं।',
  'fixity.result.missingOne': '1 अनुपस्थित है।',
  'fixity.result.missingMany': '{count} अनुपस्थित हैं।',
  'fixity.result.unreadableOne': '1 पढ़ा नहीं जा सकता।',
  'fixity.result.unreadableMany': '{count} पढ़ा नहीं जा सकता।',
  'fixity.result.allMatch': 'सभी मेल खाते हैं।',
  'fixity.result.stopped': 'जाँच अंतिम फ़ाइल से पहले रोक दी गई थी।',
  'fixity.line.problemsOne': 'अंतिम बार जाँचा गया {date}: 1 समस्या',
  'fixity.line.problemsMany': 'अंतिम बार जाँचा गया {date}: {count} समस्याएँ',
  'fixity.line.noProblems': 'अंतिम बार जाँचा गया {date}: कोई समस्या नहीं',
  'fixity.line.never': 'संग्रहीत फ़ाइलों की कभी जाँच नहीं की गई है।',
  'fixity.line.running': 'संग्रहीत फ़ाइलों की जाँच अभी की जा रही है।',
  'fixity.filter.checked': 'जाँच #{job} द्वारा पढ़ी गई फ़ाइलें।',
  'fixity.filter.ok': 'फ़ाइलें जो जाँच #{job} में मेल खाती हैं।',
  'fixity.filter.differs': 'फ़ाइलें जो जाँच #{job} में भिन्न हैं।',
  'fixity.filter.missing': 'जाँच #{job} में अनुपस्थित फ़ाइलें।',
  'fixity.filter.unreadable': 'जाँच #{job} में पढ़ी नहीं जा सकने वाली फ़ाइलें।',
  'fixity.filter.back': 'जाँच खोलें',
  'fixity.filter.clear': 'सभी मीडिया दिखाएँ',
};
