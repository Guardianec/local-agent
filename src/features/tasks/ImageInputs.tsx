import { useEffect, useState } from 'react';
import { ImagePlus, LoaderCircle, X } from 'lucide-react';
import { api } from '../../lib/ipc';
import type { ImageAttachment } from '../../generated/ImageAttachment';

export function ImagePreview({ id, name, onRemove }: { id: string; name?: string; onRemove?: () => void }) {
  const [url, setUrl] = useState(''), [failed, setFailed] = useState(false);
  useEffect(() => { let active = true; void api.imagePreview(id).then(value => { if (active) setUrl(value); }).catch(() => { if (active) setFailed(true); }); return () => { active = false; }; }, [id]);
  return <figure className="image-input">{url ? <img src={url} alt={name ?? 'Attached image'}/> : <span>{failed ? 'Preview unavailable' : <LoaderCircle className="spin" size={18}/>}</span>}<figcaption>{name ?? 'Local image'}</figcaption>{onRemove && <button className="icon-button" aria-label={`Remove ${name ?? 'image'}`} onClick={onRemove}><X size={14}/></button>}</figure>;
}

export async function ingestFiles(files: File[], source: 'drop' | 'clipboard') {
  if (files.length > 4) throw { code: 'IMAGE_COUNT', message: 'Attach up to four images.' };
  const attachments: ImageAttachment[] = [];
  for (const file of files) {
    if (file.size > 8 * 1024 * 1024) throw { code: 'IMAGE_SIZE', message: 'Choose images up to 8 MiB each.' };
    const encoded = await new Promise<string>((resolve, reject) => { const reader = new FileReader(); reader.onload = () => resolve(String(reader.result).split(',')[1] ?? ''); reader.onerror = () => reject({ message: 'The image could not be read.' }); reader.readAsDataURL(file); });
    attachments.push(await api.ingestImage(file.name || 'Screenshot.png', encoded, source));
  }
  return attachments;
}

export function ImageInputs({ images, busy, onChange, onError }: { images: ImageAttachment[]; busy: boolean; onChange: (v: ImageAttachment[]) => void; onError: (e: unknown) => void }) {
  async function choose() { try { const chosen = await api.chooseImages(); if (chosen.length + images.length > 4) throw { message: 'A task can contain up to four images.' }; onChange([...images, ...chosen]); } catch (e) { onError(e); } }
  return <div className="image-inputs"><div className="image-input-list">{images.map(image => <ImagePreview key={image.id} id={image.id} name={image.name} onRemove={busy ? undefined : () => onChange(images.filter(i => i.id !== image.id))}/>)}</div><div className="image-attach-line"><button className="text-button" disabled={busy || images.length >= 4} onClick={() => void choose()}><ImagePlus size={15}/>Attach images</button><span>{images.some(i => i.resized) ? 'Large images resized to 2,048 px. ' : ''}Paste or drop PNG, JPEG or WebP · local only</span></div></div>;
}
