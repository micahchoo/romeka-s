/**
 * The Marathi fixity check messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Marathi. Correct
 * a string here; a later run keeps it. It is typed `FixityMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { FixityMessages } from '../index';

export const mr: FixityMessages = {
  'fixity.class': 'साठवलेल्या फाईल्स तपासा',
  'fixity.start': 'साठवलेल्या फाईल्स तपासा',
  'fixity.start.note':
    'प्रत्येक साठवलेली मूळ फाईल वाचते आणि फाईल जोडली असताना नोंदवलेल्या SHA-256 शी तिची तुलना करते.',
  'fixity.start.busy': 'तपास सुरू होत आहे…',
  'fixity.start.failed': 'तपास सुरू झाला नाही: {message}',
  'fixity.result.title': 'निकाल',
  'fixity.result.loading': 'निकाल लोड होत आहे…',
  'fixity.result.failed': 'निकाल वाचता आला नाही: {message}',
  'fixity.result.waiting': 'तपास संपल्यावर येथे निकाल दाखवला जातो.',
  'fixity.result.none':
    'या तपासणीने निकाल लिहिण्यापूर्वीच तो संपवला. त्याच्या लॉगमध्ये त्याचे कारण दिले आहे.',
  'fixity.result.checkedOne': '{date} रोजी 1 फाईल तपासली गेली.',
  'fixity.result.checkedMany': '{count} फाईल्स {date} रोजी तपासल्या.',
  'fixity.result.differsOne': '1 वेगळा आहे.',
  'fixity.result.differsMany': '{count} वेगळे आहेत.',
  'fixity.result.missingOne': '1 गहाळ आहे.',
  'fixity.result.missingMany': '{count} गहाळ आहेत.',
  'fixity.result.unreadableOne': '1 वाचता येत नाही.',
  'fixity.result.unreadableMany': '{count} वाचता येत नाहीत.',
  'fixity.result.allMatch': 'सर्व जुळले.',
  'fixity.result.stopped': 'शेवटच्या फाईलपूर्वी तपासणी थांबवण्यात आली.',
  'fixity.line.problemsOne': 'शेवटची तपासलेली फाईल: {date}: 1 समस्या',
  'fixity.line.problemsMany': 'शेवटची तपासलेली फाईल: {date}: {count} समस्या',
  'fixity.line.noProblems': 'शेवटची तपासलेली फाईल: {date}: कोणतीही समस्या नाही',
  'fixity.line.never': 'साठवलेल्या फाईल्सची कधीही तपासणी केलेली नाही.',
  'fixity.line.running': 'साठवलेल्या फाईल्सची आता तपासणी केली जात आहे.',
  'fixity.filter.checked': '#{job} तपासणीने वाचलेल्या फाईल्स.',
  'fixity.filter.ok': '#{job} तपासणीत जुळलेल्या फाईल्स.',
  'fixity.filter.differs': '#{job} तपासणीत भिन्न असलेल्या फाईल्स.',
  'fixity.filter.missing': '#{job} तपासणीत गहाळ असलेल्या फाईल्स.',
  'fixity.filter.unreadable': '#{job} तपासणीत वाचता न आलेल्या फाईल्स.',
  'fixity.filter.back': 'तपासणी उघडा',
  'fixity.filter.clear': 'सर्व मीडिया दाखवा',
};
