/**
 * The Tamil media notes panel messages.
 *
 * @machine-translated: written by `scripts/translate-locales.mjs` with sarvam-30b and
 * md-translator's checks, and not yet reviewed by a reader of Tamil. Correct
 * a string here; a later run keeps it. It is typed `AnnotationsMessages`, so a key
 * added to English and not here is a `tsc` error.
 */
import type { AnnotationsMessages } from '../index';

export const ta: AnnotationsMessages = {
  'notes.heading': 'குறிப்புகள்',
  'notes.loading': 'குறிப்புகள் ஏற்றப்படுகின்றன...',
  'notes.listLabel': 'இந்த ஊடகத்தில் உள்ள குறிப்புகள்',
  'notes.facts.image': '{total} குறிப்புகள். {regions} பகுதிகளில், {whole} முழுமையான படத்தில்.',
  'notes.facts.imageOne': '1 குறிப்பு. {regions} ஒரு பகுதியில், {whole} முழுமையான படத்தில்.',
  'notes.facts.time': '{total} குறிப்புகள். {times} ஒரு நேரத்தில், {whole} முழுமையான கோப்பில்.',
  'notes.facts.timeOne': '1 குறிப்பு. {times} ஒரு நேரத்தில், {whole} முழுமையான கோப்பில்.',
  'notes.facts.file': 'இந்த கோப்பில் {total} குறிப்புகள்.',
  'notes.facts.fileOne': 'இந்த கோப்பில் 1 குறிப்பு.',
  'notes.readOnly':
    'இந்த குறிப்புகளை நீங்கள் வாசிக்கலாம். அவற்றை மாற்றுவதற்கு இந்த ஊடகத்தில் திருத்தும் உரிமைகள் தேவை.',
  'notes.empty.image':
    'இன்னும் குறிப்புகள் இல்லை. முழுமையான படத்தின் மீது ஒரு குறிப்பைச் சேர்க்கவும்.',
  'notes.empty.time':
    'இன்னும் குறிப்புகள் இல்லை. முழுமையான பதிவைப் பற்றி ஒரு குறிப்பைச் சேர்க்கவும்.',
  'notes.empty.file': 'இன்னும் குறிப்புகள் இல்லை.',
  'notes.add.image': 'முழுமையான படத்தின் ஒரு குறிப்பைச் சேர்க்கவும்',
  'notes.add.time': 'முழுமையான பதிவைப் பற்றி ஒரு குறிப்பைச் சேர்க்கவும்',
  'notes.add.file': 'இந்த கோப்பைப் பற்றி ஒரு குறிப்பைச் சேர்க்கவும்',
  'notes.region': 'பகுதி {number}',
  'notes.timeRange': '{start} முதல் {end} வரை',
  'notes.wholeImage': 'முழுமையான படம்',
  'notes.wholeFile': 'முழுமையான கோப்பு',
  'notes.wholeTime': 'முழுமையான பதிவு',
  'notes.fromTrack':
    'தலைப்பு கோப்பிலிருந்து. இந்த தலைப்புகளின் அடுத்த இறக்குமதி இந்த குறிப்பை மாற்றீடு செய்யும்.',
  'notes.changed': 'மாற்றப்பட்டது {when}',
  'notes.noText': '(உரை இல்லை)',
  'notes.draft': 'சேமிக்கப்படாத',
  'notes.draftBanner':
    'உங்களிடம் சேமிக்கப்படாத ஒரு குறிப்பு உள்ளது. அதை வைத்துக்கொள்ளலாம், அல்லது அதை நீக்கிவிடலாம்.',
  'notes.keep': 'இதைப் பாதுகாக்கவும்',
  'notes.discard': 'நீக்கவும்',
  'notes.restoredChange': 'உங்கள் சேமிக்கப்படாத மாற்றம் மீண்டும் வந்துவிட்டது.',
  'notes.filterLabel': 'உரையின் அடிப்படையில் குறிப்புகளை வடிகட்டவும்',
  'notes.filterPlaceholder': 'உரையின் அடிப்படையில் வடிகட்டவும்',
  'notes.filter.region': 'பகுதிகள்',
  'notes.filter.time': 'நேரங்கள்',
  'notes.filter.whole': 'முழு கோப்பு',
  'notes.clearFilter': 'வடிகட்டியை நீக்கவும்',
  'notes.matchCount': '{total} குறிப்புகளில் {shown} “{query}” உடன் பொருந்துகின்றன.',
  'notes.shownCount': '{total} குறிப்புகளில் {shown} காட்டப்படுகின்றன',
  'notes.showMore': 'மேலும் 50-ஐக் காட்டவும்',
  'notes.noMatch': 'வடிகட்டியுடன் பொருந்தக்கூடிய குறிப்புகள் எதுவும் இல்லை.',
  'notes.form.newTitle': 'ஒரு புதிய குறிப்பு',
  'notes.form.editTitle': 'இந்த குறிப்பைத் திருத்தவும்',
  'notes.form.text': 'குறிப்பு',
  'notes.form.language': 'மொழி',
  'notes.form.motivation': 'குறிப்பு என்ன செய்கிறது',
  'notes.motivation.commenting': 'ஒரு கருத்து',
  'notes.motivation.tagging': 'ஒரு குறிச்சொல்',
  'notes.motivation.describing': 'ஒரு விளக்கம்',
  'notes.motivation.supplementing': 'ஒரு உரைப்பகுதி',
  'notes.form.advanced': 'மேம்பட்ட',
  'notes.form.raw': 'சர்வர் எதைச் சேமிக்கிறது',
  'notes.form.saving': 'சேமிக்கப்படுகிறது...',
  'notes.form.savingLong':
    'இன்னும் சேமிக்கப்படுகிறது. உங்கள் குறிப்பு இங்கே வைக்கப்படுகிறது. நீங்கள் தொடர்ந்து பணியாற்றலாம்.',
  'notes.form.empty': 'முதலில் குறிப்பைத் தட்டச்சு செய்யவும்.',
  'notes.form.noChanges': 'மாற்றங்கள் ஏதுமில்லை.',
  'notes.form.delete': 'நீக்கு',
  'notes.form.deleteConfirm': 'இந்த குறிப்பை நீக்கவும்',
  'notes.form.keep': 'சேமி',
  'notes.unknownMotivation':
    'இந்த குறிப்பு ஒரு உந்துதலைப் பயன்படுத்துகிறது, இதை இந்தப் பக்கத்தால் மாற்ற முடியாது.',
  'notes.saved': 'சேமிக்கப்பட்டது.',
  'notes.deleted': 'நீக்கப்பட்டது.',
  'notes.alreadyDeleted': 'இந்த குறிப்பு ஏற்கனவே நீக்கப்பட்டது.',
  'notes.keptTheirs': 'அவர்களின் பதிப்பைத் தக்கவைத்துக் கொண்டனர்.',
  'notes.notSaved':
    'சேமிக்கப்படவில்லை. உங்கள் மாற்றம் தக்கவைக்கப்பட்டுள்ளது. மீண்டும் முயற்சிக்கவும்.',
  'notes.conflict': 'முரண்பாடு',
  'notes.conflict.save':
    'வேறு யாரோ இந்த குறிப்பை மாற்றியுள்ளனர். அவர்களின் பதிப்பு இங்கே காட்டப்பட்டுள்ளது. உங்கள் உரை கீழே உள்ளது.',
  'notes.conflict.delete':
    'இந்த குறிப்பைத் திறந்த பிறகு யாரோ ஒருவர் அதை மாற்றியுள்ளார். அவர்களின் பதிப்பு இங்கே காட்டப்பட்டுள்ளது.',
  'notes.conflict.theirs': 'அவர்களின் பதிப்பு',
  'notes.conflict.yours': 'உங்கள் உரை',
  'notes.conflict.saveMine': 'அவர்களின் பதிப்பை விட எனது பதிப்பைச் சேமிக்கவும்',
  'notes.conflict.useTheirs': 'அவர்களின் பதிப்பைப் பயன்படுத்தவும்',
  'notes.conflict.deleteTheirs': 'அவர்களின் பதிப்பை நீக்கவும்',
  'notes.gone':
    'இந்த குறிப்பு நீக்கப்பட்டது. உங்கள் உரை பாதுகாக்கப்படுகிறது. இதை ஒரு புதிய குறிப்பாகச் சேமிக்கவா?',
  'notes.gone.saveAsNew': 'புதிய குறிப்பாகச் சேமிக்கவும்',
  'notes.stale': 'எண்ணிக்கைகள் காலாவதியாக இருக்கலாம்.',
  'notes.refresh': 'புதுப்பித்தல்',
  'notes.loadFailed': 'குறிப்புகள் ஏற்றப்படவில்லை.',
  'notes.retry': 'மீண்டும் முயற்சிக்கவும்',
  'region.label': 'படம் மற்றும் அதன் பகுதிகள்',
  'region.tools': 'பகுதி கருவிகள்',
  'region.tool.move': 'நகர்த்து',
  'region.tool.rect': 'செவ்வகம்',
  'region.tool.polygon': 'பலகோணம்',
  'region.advanced': 'மேம்பட்ட',
  'region.hint.move':
    'வடிவத்தை மாற்ற ஒரு வடிவத்தைத் தேர்ந்தெடுக்கவும். Shift மற்றும் இழுப்பது ஒரு செவ்வகத்தைக் குறிக்கிறது, மேலும் அம்புக்குறிகள் தேர்ந்தெடுக்கப்பட்ட வடிவத்தை நகர்த்தும் அதே வேளையில், படத்தின் மீது கவனத்தை வைத்திருக்கும்.',
  'region.hint.rect': 'ஒரு செவ்வகத்தைக் குறிக்க படத்தில் இழுக்கவும். Esc ரத்து செய்கிறது.',
  'region.hint.polygon':
    'ஒவ்வொரு மூலையையும் கிளிக் செய்யவும். உள்ளிடவும் அல்லது முதல் மூலையில் கிளிக் செய்வது வடிவத்தை மூடுகிறது. Backspace கடைசி மூலையை நீக்குகிறது.',
  'region.zoom.in': 'பெரிதாக்கு',
  'region.zoom.out': 'சுருக்கு',
  'region.zoom': '{zoom}×',
  'region.hide': 'வடிவங்களை மறை',
  'region.show': 'வடிவங்களைக் காட்டு',
  'region.canvas': 'படம். அதன் மீது ஒரு பகுதியை வரையவும்.',
  'region.draft': 'சேமிக்கப்படாத',
  'region.corners': '{count} இல் {max} மூலைகள்',
  'region.tooMany': 'ஒரு வடிவம் அதிகபட்சம் {max} மூலைகளைக் கொண்டிருக்கலாம்.',
  'region.tooFew': 'ஒரு வடிவத்திற்கு குறைந்தது மூன்று மூலைகள் தேவை.',
  'region.selected': 'பகுதி {number}: x {x}, y {y}, {width} பை {height} பிக்சல்கள்.',
  'region.byNumbers.title': 'எண்களால் ஒரு பகுதியைச் சேர்க்கவும்',
  'region.byNumbers.hint': 'படத்தில் அதன் சொந்த பிக்சல்களில் ({width} × {height}).',
  'region.byNumbers.x': 'இடது',
  'region.byNumbers.y': 'மேல்',
  'region.byNumbers.width': 'அகலம்',
  'region.byNumbers.height': 'உயரம்',
  'region.byNumbers.add': 'பகுதியைச் சேர்க்கவும்',
  'region.byNumbers.invalid':
    'ஒரு பகுதிக்கு முழு எண்கள் தேவை, குறைந்தபட்சம் 1 பை 1 பிக்சல், மேலும் அது படத்தில் பொருந்த வேண்டும்.',
};
