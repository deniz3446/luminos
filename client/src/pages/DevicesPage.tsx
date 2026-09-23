import { useCallback, useEffect, useState } from "react";
import { apiJson } from "../api/client";
import { ErrorBox, FeaturePage, MetricCard, EmptyState, StatusPill } from "../components/FeatureShell";
import { formatBytes, formatDate } from "../utils/featureFormat";

type Device={device_id:string;device_name:string;total_media_count:number;photo_count:number;video_count:number;total_bytes:number;ip_address?:string|null;network_scope?:string|null;last_filename?:string|null;last_source_type?:string|null;last_upload_at?:string|null;online:boolean};
type Pairing={code?:string;expires_at?:number|string;expires_in_seconds?:number};
export default function DevicesPage(){const[items,setItems]=useState<Device[]>([]);const[error,setError]=useState("");const[pairing,setPairing]=useState<Pairing|null>(null);const[loading,setLoading]=useState(true);
 const load=useCallback(async()=>{setLoading(true);setError("");try{setItems(await apiJson<Device[]>("/api/v1/devices"));}catch(e){setError(e instanceof Error?e.message:String(e));}finally{setLoading(false);}},[]);useEffect(()=>{const timer=window.setTimeout(()=>void load(),0);return()=>window.clearTimeout(timer);},[load]);
 const createPairing=async()=>{setError("");try{setPairing(await apiJson<Pairing>("/api/v1/mobile/pairing",{method:"POST"}));}catch(e){setError(e instanceof Error?e.message:String(e));}};
 return <FeaturePage title="Cihazlar" subtitle="PhotoOS Mobile cihazları ve son aktarım bilgileri" actions={<><button className="feature-button" onClick={()=>void load()} disabled={loading}>Yenile</button><button className="feature-button primary" onClick={()=>void createPairing()}>Telefon eşleştir</button></>}>
 {error&&<ErrorBox message={error}/>} {pairing?.code&&<div className="feature-card" style={{marginBottom:16}}><strong>Eşleştirme kodu</strong><div className="code-box">{pairing.code}</div><small>Kod yaklaşık {pairing.expires_in_seconds??600} saniye geçerlidir.</small></div>}
 <div className="metric-grid"><MetricCard label="Cihaz" value={items.length}/><MetricCard label="Çevrimiçi" value={items.filter(x=>x.online).length}/><MetricCard label="Medya" value={items.reduce((a,b)=>a+b.total_media_count,0)}/></div>
 {items.length===0?<EmptyState>{loading?"Yükleniyor…":"Kayıtlı cihaz yok."}</EmptyState>:<div className="feature-table-wrap"><table className="feature-table"><thead><tr><th>Cihaz</th><th>Durum</th><th>Medya</th><th>Boyut</th><th>Ağ</th><th>Son aktarım</th></tr></thead><tbody>{items.map(d=><tr key={d.device_id}><td><strong>{d.device_name}</strong><br/><small>{d.last_filename??d.device_id}</small></td><td><StatusPill ok={d.online} label={d.online?"Online":"Offline"}/></td><td>{d.total_media_count} ({d.photo_count} foto / {d.video_count} video)</td><td>{formatBytes(d.total_bytes)}</td><td>{d.ip_address??"—"}<br/><small>{d.network_scope??""}</small></td><td>{formatDate(d.last_upload_at)}</td></tr>)}</tbody></table></div>}
 </FeaturePage>}
