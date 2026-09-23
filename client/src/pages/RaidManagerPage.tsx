import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "../api/client";
import "./RaidManagerPage.css";

type RaidArray = { device:string; name:string; level:string; state:string; active_devices:number; total_devices:number; failed_devices:number; spare_devices:number; sync_action:string|null; sync_percent:number|null; detail:string };
type RaidStatus = { configured:boolean; healthy:boolean; status:string; arrays:RaidArray[]; mdstat:string };
type Candidate = { device:string; model:string; serial:string; size_bytes:number; mounted:boolean; mountpoints:string[]; system_disk:boolean; photoos_disk:boolean; eligible:boolean; reason:string };
type Plan = { valid:boolean; destructive:boolean; level:string; devices:string[]; array_device:string; estimated_usable_bytes:number; warnings:string[]; command_preview:string; execution_enabled:boolean };

function bytes(value:number){ if(value>=1e12)return `${(value/1e12).toFixed(2)} TB`; if(value>=1e9)return `${(value/1e9).toFixed(2)} GB`; return `${(value/1e6).toFixed(0)} MB`; }

export default function RaidManagerPage(){
 const [status,setStatus]=useState<RaidStatus|null>(null); const [disks,setDisks]=useState<Candidate[]>([]); const [selected,setSelected]=useState<string[]>([]); const [level,setLevel]=useState("raid1"); const [plan,setPlan]=useState<Plan|null>(null); const [error,setError]=useState(""); const [loading,setLoading]=useState(true);
 const load=useCallback(async()=>{ try{ const [s,c]=await Promise.all([api(`/api/v1/raid/status?ts=${Date.now()}`),api(`/api/v1/raid/candidates?ts=${Date.now()}`)]); if(!s.ok||!c.ok)throw new Error("RAID API yanıt vermedi"); setStatus(await s.json()); const cj=await c.json(); setDisks(cj.disks??[]); setError(""); }catch(e){setError(e instanceof Error?e.message:"RAID verisi alınamadı")}finally{setLoading(false)} },[]);
 useEffect(()=>{void load(); const t=setInterval(()=>void load(),5000); return()=>clearInterval(t)},[load]);
 const eligible=useMemo(()=>disks.filter(d=>d.eligible),[disks]);
 function toggle(device:string){setSelected(v=>v.includes(device)?v.filter(x=>x!==device):[...v,device]);setPlan(null)}
 async function buildPlan(){setError(""); const r=await api("/api/v1/raid/plan",{method:"POST",headers:{"Content-Type":"application/json"},body:JSON.stringify({level,devices:selected,array_name:"md0"})}); const j=await r.json(); if(!r.ok){setError(j.error??"Plan oluşturulamadı");return} setPlan(j)}
 if(loading)return <div className="raid-loading">RAID Manager yükleniyor…</div>;
 return <div className="raid-page">
  <header className="raid-hero"><div><span className="eyebrow">PHOTOOS STORAGE</span><h2>RAID Manager</h2><p>mdadm dizilerini izle, disk uygunluğunu denetle ve güvenli kurulum planı oluştur.</p></div><button onClick={()=>void load()}>↻ Yenile</button></header>
  {error&&<div className="raid-error">⚠ {error}</div>}
  <section className="raid-summary">
   <article><span>RAID durumu</span><strong className={status?.healthy?"ok":"warn"}>{status?.configured?(status.healthy?"Sağlıklı":"Dikkat"):"Yapılandırılmamış"}</strong><small>{status?.status}</small></article>
   <article><span>Dizi sayısı</span><strong>{status?.arrays.length??0}</strong><small>Linux MD aygıtı</small></article>
   <article><span>Uygun boş disk</span><strong>{eligible.length}</strong><small>Bağlı olmayan disk</small></article>
   <article><span>Güvenlik modu</span><strong className="ok">Planlama</strong><small>Yıkıcı işlem kapalı</small></article>
  </section>
  <section className="raid-panel"><div className="panel-title"><div><h3>Mevcut RAID dizileri</h3><p>/proc/mdstat üzerinden canlı izleme</p></div></div>
   {!status?.configured?<div className="raid-empty">💽 <b>RAID yapılandırılmamış</b><span>PHOTOOS_DATA1 ve PHOTOOS_DATA2 bağımsız disk olarak çalışıyor.</span></div>:status.arrays.map(a=><article className="array-card" key={a.device}><div><b>{a.device}</b><span>{a.level.toUpperCase()} · {a.active_devices}/{a.total_devices} disk</span></div><div className={a.failed_devices===0?"pill ok":"pill bad"}>{a.failed_devices===0?"Healthy":`${a.failed_devices} arızalı`}</div>{a.sync_action&&<div className="sync"><span>{a.sync_action}</span><progress value={a.sync_percent??0} max="100"/><b>{a.sync_percent?.toFixed(1)??0}%</b></div>}</article>)}
  </section>
  <section className="raid-grid"><div className="raid-panel"><div className="panel-title"><div><h3>Fiziksel diskler</h3><p>Sistem ve PhotoOS veri diskleri koruma altında</p></div></div><div className="disk-list">{disks.map(d=><label className={`disk-row ${d.eligible?"eligible":"locked"}`} key={d.device}><input type="checkbox" disabled={!d.eligible} checked={selected.includes(d.device)} onChange={()=>toggle(d.device)}/><div className="disk-icon">◉</div><div className="disk-main"><b>{d.device} · {d.model}</b><span>{bytes(d.size_bytes)} {d.serial&&`· ${d.serial}`}</span><small>{d.reason}</small></div><span className={`pill ${d.eligible?"ok":"neutral"}`}>{d.eligible?"Uygun":"Korumalı"}</span></label>)}</div></div>
  <div className="raid-panel builder"><div className="panel-title"><div><h3>RAID planı</h3><p>Gerçek işleme geçmeden kapasite ve komutu doğrula</p></div></div><label>RAID seviyesi<select value={level} onChange={e=>{setLevel(e.target.value);setPlan(null)}}><option value="raid0">RAID 0</option><option value="raid1">RAID 1</option><option value="raid5">RAID 5</option><option value="raid6">RAID 6</option><option value="raid10">RAID 10</option></select></label><div className="selected-box"><span>Seçilen diskler</span><b>{selected.length}</b><small>{selected.join(", ")||"Henüz uygun disk seçilmedi"}</small></div><button className="plan-btn" disabled={selected.length===0} onClick={()=>void buildPlan()}>Planı Oluştur</button>{plan&&<div className="plan-result"><h4>{plan.level.toUpperCase()} · {bytes(plan.estimated_usable_bytes)}</h4>{plan.warnings.map(w=><p key={w}>⚠ {w}</p>)}<code>{plan.command_preview}</code><button disabled>RAID Oluşturma V1'de Kilitli</button></div>}</div></section>
  <section className="raid-panel safety"><h3>Güvenlik notu</h3><p>Mevcut PHOTOOS_DATA1 ve PHOTOOS_DATA2 disklerinde veri bulunduğu için web üzerinden RAID oluşturma bilerek kapalıdır. RAID1’e geçiş; doğrulanmış yedek, boş diskler ve planlı veri taşıma süreci gerektirir.</p></section>
 </div>
}
