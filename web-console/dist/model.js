export const defaults = Object.freeze({deviceName:'Terra',rotation:'0',volumeStep:'2',autoConnect:true,storageName:'HDD laptop',filesystem:'ntfs',wifiName:'',remoteMode:'undecided'});
const choices = {rotation:['0','90','180','270'],volumeStep:['1','2','5'],filesystem:['ntfs','fat32-test'],remoteMode:['undecided','vercel-relay','cloud-relay','cloudflare-free','router-vpn','lan']};
export function validateConfig(value) {
  if (!value || typeof value !== 'object' || Array.isArray(value)) throw new Error('Draf tidak valid.');
  const result = {};
  for (const [key,max] of [['deviceName',31],['storageName',40],['wifiName',32]]) {
    if (typeof value[key] !== 'string' || /[\u0000-\u001f\u007f]/u.test(value[key])) throw new Error('Teks konfigurasi tidak valid.');
    const text = value[key].trim();
    if ((key !== 'wifiName' && !text) || new TextEncoder().encode(text).length > max) throw new Error(`${key}: maksimum ${max} byte UTF-8, nama wajib diisi.`);
    result[key] = text;
  }
  for (const [key,allowed] of Object.entries(choices)) {
    if (!allowed.includes(value[key])) throw new Error('Pilihan konfigurasi tidak dikenali.');
    result[key] = value[key];
  }
  if (typeof value.autoConnect !== 'boolean') throw new Error('Status auto connect tidak valid.');
  result.autoConnect = value.autoConnect;
  return result;
}
export const files = [
  {id:'documents',parent:'root',name:'Dokumen',type:'folder',date:'2026-10-11'},
  {id:'music',parent:'root',name:'Musik',type:'folder',date:'2026-10-10'},
  {id:'notes',parent:'root',name:'Catatan perangkat',type:'folder',date:'2026-10-09'},
  {id:'welcome',parent:'root',name:'Mulai di sini.txt',type:'file',date:'2026-10-11',content:'Selamat datang di Terra Home Hub.\n\nIni file contoh untuk mencoba preview dan unduh.\nTidak ada file dari HDD Anda yang diakses.\n\nHardware: ESP32-S3, ST7789 240×240, HDD USB 500 GB NTFS.\nNAS dan relay belum diimplementasikan.'},
  {id:'readme',parent:'documents',name:'Rencana penyimpanan.txt',type:'file',date:'2026-10-11',content:'DRAF RENCANA\n\n1. Periksa daya HDD dan USB MSC.\n2. Uji read-only dengan media FAT32 terpisah.\n3. Putuskan filesystem HDD utama.\n4. Tambahkan web dan relay setelah LAN stabil.\n\nTidak ada format otomatis.'},
  {id:'playlist',parent:'music',name:'Daftar putar contoh.txt',type:'file',date:'2026-10-10',content:'DAFTAR PUTAR CONTOH\n\nTest Song — Test Artist\n\nIni daftar teks, bukan file audio. Playback Windows akan diintegrasikan melalui BLE pada milestone berikutnya.'},
  {id:'wiring',parent:'notes',name:'Wiring LCD.txt',type:'file',date:'2026-10-09',content:'ST7789 240×240\n\nSCLK: GPIO12\nMOSI: GPIO11\nCS: tidak ada (modul 7 pin)\nSPI mode 3\nDC: GPIO9\nRST: GPIO8\nBL enable: GPIO7\n\nLogika 3,3 V. BL langsung LED membutuhkan driver yang sesuai.\nGap default 0/0 perlu diuji pada panel fisik.'}
];
export function listFiles(parent='root',query='',kind='all',sort='name') {
  return files.filter(file=>file.parent===parent && (kind==='all'||file.type===kind) && file.name.toLocaleLowerCase('id').includes(query.toLocaleLowerCase('id')))
    .sort((a,b)=>a.type!==b.type ? (a.type==='folder'?-1:1) : sort==='newest' ? b.date.localeCompare(a.date)||a.name.localeCompare(b.name,'id') : a.name.localeCompare(b.name,'id'));
}
