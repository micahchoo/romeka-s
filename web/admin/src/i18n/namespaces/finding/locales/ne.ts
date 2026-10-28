/**
 * The Nepali jump-to box and saved list messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Nepali. Correct
 * a string here; a later run keeps it. It is typed `FindingMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { FindingMessages } from '../index';

export const ne: FindingMessages = {
  'jump.loading': '“{text}” खोज्दै',
  'jump.none': 'तपाईँले देख्न सक्ने कुनै पनि रेकर्ड “{text}” सँग मेल खाँदैन।',
  'jump.many': '“{text}” सँग मेल खाने {count} रेकर्डहरू',
  'jump.firstTen':
    'पहिलो {count} सूचीबद्ध छन्। तिनीहरूलाई साँघुरो बनाउन शीर्षकको थप भाग टाइप गर्नुहोस्।',
  'jump.failed': 'खोज पुरा भएन: {message}',
  'jump.close': 'बन्द गर्नुहोस्',
  'jump.kind.items': 'वस्तु',
  'jump.kind.item_sets': 'वस्तु समूह',
  'jump.kind.media': 'मिडिया',
  'jump.untitled': '[अनाम]',
  'savedLists.save': 'यस सूचीलाई सुरक्षित गर्नुहोस्',
  'savedLists.name': 'नाम',
  'savedLists.where':
    'यसले यी फिल्टर र क्रम सुरक्षित गर्छ। नाम मेनुमा रहेका वस्तुहरू मुनि, तपाईँले साइन इन गर्ने जुनसुकै कम्प्युटरमा देखिनेछ।',
  'savedLists.submit': 'सुरक्षित',
  'savedLists.saving': 'सुरक्षित गर्दै…',
  'savedLists.cancel': 'रद्द',
  'savedLists.current': 'सुरक्षित सूची: {name}',
  'savedLists.rename': 'सुरक्षित सूचीको नाम परिवर्तन गर्नुहोस्',
  'savedLists.delete': 'सुरक्षित सूची मेट्नुहोस्',
  'savedLists.deleted': 'सुरक्षित सूची {name} मेटाइयो।',
  'savedLists.nameEmpty': 'सूचीलाई एउटा नाम दिनुहोस्।',
  'savedLists.nameTaken': 'तपाईँसँग पहिले नै {name} नामको सूची छ।',
  'savedLists.tooMany':
    'तपाईँसँग {count} वटा सुरक्षित सूचीहरू छन्। यो एक व्यक्तिले राख्न सक्ने सबैभन्दा धेरै हो। पहिले एउटा मेटाउनुहोस्।',
  'savedLists.gone': 'यो सुरक्षित सूची अर्को विन्डोमा मेटाइएको थियो।',
};
