/**
 * The Nepali fixity check messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Nepali. Correct
 * a string here; a later run keeps it. It is typed `FixityMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { FixityMessages } from '../index';

export const ne: FixityMessages = {
  'fixity.class': 'भण्डारण गरिएका फाइलहरू जाँच गर्नुहोस्',
  'fixity.start': 'भण्डारण गरिएका फाइलहरू जाँच गर्नुहोस्',
  'fixity.start.note':
    'यसले प्रत्येक भण्डारण गरिएको मौलिक प्रतिलिपि पढ्छ र फाइल थप्दा रेकर्ड गरिएको SHA-256 सँग तुलना गर्छ।',
  'fixity.start.busy': 'जाँच सुरु गर्दै…',
  'fixity.start.failed': 'जाँच सुरु भएन: {message}',
  'fixity.result.title': 'परिणाम',
  'fixity.result.loading': 'परिणाम लोड गर्दै…',
  'fixity.result.failed': 'परिणाम पढ्न सकिएन: {message}',
  'fixity.result.waiting': 'जाँच समाप्त भएपछि यहाँ परिणाम देखिनेछ।',
  'fixity.result.none': 'यसले परिणाम लेख्नु अघि नै जाँच समाप्त भयो। यसको लगले किन भयो भनी बताउँछ।',
  'fixity.result.checkedOne': '{date} मा 1 फाइल जाँच गरियो।',
  'fixity.result.checkedMany': '{count} फाइलहरू {date} मा जाँच गरिए।',
  'fixity.result.differsOne': '1 फरक छ।',
  'fixity.result.differsMany': '{count} फरक छन्।',
  'fixity.result.missingOne': '1 हराइरहेको छ।',
  'fixity.result.missingMany': '{count} हराइरहेका छन्।',
  'fixity.result.unreadableOne': '1 पढ्न सकिएन।',
  'fixity.result.unreadableMany': '{count} पढ्न सकिएन।',
  'fixity.result.allMatch': 'सबै मेल खान्छन्।',
  'fixity.result.stopped': 'जाँच अन्तिम फाइल भन्दा अगाडि रोकियो।',
  'fixity.line.problemsOne': 'अन्तिम पटक जाँच गरिएको फाइल {date}: 1 समस्या',
  'fixity.line.problemsMany': 'अन्तिम पटक जाँच गरिएको फाइल {date}: {count} समस्याहरू',
  'fixity.line.noProblems': 'अन्तिम पटक जाँच गरिएको फाइल {date}: कुनै समस्या छैन',
  'fixity.line.never': 'भण्डारण गरिएका फाइलहरू कहिल्यै जाँच गरिएका छैनन्।',
  'fixity.line.running': 'भण्डारण गरिएका फाइलहरू अहिले जाँच गर्दै।',
  'fixity.filter.checked': 'जाँच #{job} द्वारा पढिएका फाइलहरू।',
  'fixity.filter.ok': 'जाँच #{job} मा मेल खाएका फाइलहरू।',
  'fixity.filter.differs': 'जाँच #{job} मा फरक भएका फाइलहरू।',
  'fixity.filter.missing': 'जाँच #{job} मा छुटेका फाइलहरू।',
  'fixity.filter.unreadable': 'जाँच #{job} मा पढ्न नसकिने फाइलहरू।',
  'fixity.filter.back': 'जाँच खोल्नुहोस्',
  'fixity.filter.clear': 'सबै मिडिया देखाउनुहोस्',
};
