/**
 * The Hindi page history messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Hindi. Correct
 * a string here; a later run keeps it. It is typed `PageHistoryMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { PageHistoryMessages } from '../index';

export const hi: PageHistoryMessages = {
  'history.title': 'पृष्ठ इतिहास',
  'history.close': 'बंद करें',
  'history.loading': 'संस्करण लोड किए जा रहे हैं...',
  'history.empty': 'अभी तक कोई संस्करण नहीं। प्रत्येक सहेजने पर एक सुरक्षित रहता है।',
  'history.entry': '{time} · {name} · {blocks}',
  'history.today': 'आज {time}',
  'history.notRecorded': 'लेखक दर्ज नहीं किया गया',
  'history.deletedUser': 'हटाए गए उपयोगकर्ता',
  'history.firstVersion': 'पहला सहेजा गया संस्करण',
  'history.created': 'बनाया गया',
  'history.olderGone': 'पुराने संस्करण अब नहीं रखे जाते',
  'history.noChanges': 'कोई बदलाव नहीं',
  'history.retitled': 'शीर्षक बदलकर “{before}” से “{after}” कर दिया गया',
  'history.settingsChanged': 'पृष्ठ सेटिंग्स बदल दी गई हैं',
  'history.addedOne': '1 ब्लॉक जोड़ा गया',
  'history.added': '{count} ब्लॉक जोड़े गए',
  'history.removedOne': '1 ब्लॉक हटाया गया',
  'history.removed': '{count} ब्लॉक हटाए गए',
  'history.changedOne': '1 ब्लॉक बदला गया',
  'history.changed': '{count} ब्लॉक बदले गए',
  'history.changedNamedOne': '1 ब्लॉक बदला गया: {names}',
  'history.changedNamed': '{count} ब्लॉक बदले गए: {names}',
  'history.movedOne': '1 ब्लॉक स्थानांतरित किया गया',
  'history.moved': '{count} ब्लॉक स्थानांतरित किए गए',
  'history.live': 'अभी लाइव',
  'history.viewing': '{time} का संस्करण देख रहे हैं',
  'history.restore': 'इस संस्करण को पुनर्स्थापित करें',
  'history.back': 'वर्तमान ड्राफ्ट पर वापस जाएँ',
  'history.rendering': 'इस संस्करण को रेंडर किया जा रहा है...',
  'history.renderFailed': 'इस संस्करण को रेंडर नहीं किया जा सका।',
  'history.conflictTitle': 'किसी और ने इस पृष्ठ को सहेजा है',
  'history.conflict': 'आपके यह पृष्ठ खोलने के बाद {name} ने इसे {time} पर सहेजा',
  'history.conflictUntimed': 'आपके यह पृष्ठ खोलने के बाद {name} ने इसे सहेजा',
  'history.conflictChoices':
    'उनका संस्करण संपादक में खुलेगा, और आपका संस्करण “पूर्ववत करें” से वापस आ जाएगा। “अपना रखें” आपके संस्करण को उनके संस्करण के ऊपर सहेजता है।',
  'history.someoneElse': 'कोई और',
  'history.showTheirs': 'उनका संस्करण दिखाएँ',
  'history.keepMine': 'अपना रखें',
};
