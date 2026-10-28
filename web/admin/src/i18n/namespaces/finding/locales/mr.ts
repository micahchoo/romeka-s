/**
 * The Marathi jump-to box and saved list messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Marathi. Correct
 * a string here; a later run keeps it. It is typed `FindingMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { FindingMessages } from '../index';

export const mr: FindingMessages = {
  'jump.loading': '“{text}” शोधत आहे…',
  'jump.none': '“{text}” शी जुळणारी कोणतीही नोंद दिसत नाही.',
  'jump.many': '“{text}” शी जुळणारे {count} रेकॉर्ड्स',
  'jump.firstTen':
    'पहिले {count} सूचीबद्ध आहेत. त्यांना मर्यादित करण्यासाठी शीर्षकाचा अधिक भाग टाईप करा.',
  'jump.failed': 'शोध पूर्ण झाला नाही: {message}',
  'jump.close': 'बंद करा',
  'jump.kind.items': 'घटक',
  'jump.kind.item_sets': 'घटक संच',
  'jump.kind.media': 'माध्यम',
  'jump.untitled': '[शीर्षकहीन]',
  'savedLists.save': 'ही सूची जतन करा',
  'savedLists.name': 'नाव',
  'savedLists.where':
    'हे फिल्टर्स आणि हा क्रम जतन करते. नाव मेनूमध्ये आयटम्स अंतर्गत, तुम्ही ज्या कोणत्याही संगणकावर साइन इन करता तिथे दिसते.',
  'savedLists.submit': 'जतन करा',
  'savedLists.saving': 'जतन करत आहे…',
  'savedLists.cancel': 'रद्द करा',
  'savedLists.current': 'जतन केलेली सूची: {name}',
  'savedLists.rename': 'जतन केलेल्या सूचीचे नाव बदला',
  'savedLists.delete': 'जतन केलेली सूची हटवा',
  'savedLists.deleted': 'जतन केलेली सूची {name} हटवली.',
  'savedLists.nameEmpty': 'सूचीला एक नाव द्या.',
  'savedLists.nameTaken': 'तुमच्याकडे आधीच {name} नावाची सूची आहे.',
  'savedLists.tooMany':
    'तुमच्याकडे {count} जतन केलेल्या सूची आहेत. एक व्यक्ती जास्तीत जास्त इतक्याच सूची ठेवू शकते. प्रथम एक हटवा.',
  'savedLists.gone': 'ही जतन केलेली सूची दुसऱ्या खिडकीत हटवण्यात आली आहे.',
};
