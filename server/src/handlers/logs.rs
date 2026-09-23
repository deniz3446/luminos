use std::{collections::HashSet, path::Path, time::{SystemTime, UNIX_EPOCH}};
use axum::{extract::Query, http::StatusCode, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::process::Command;

const JOURNALCTL: &str = "/usr/bin/journalctl";

#[derive(Debug, Clone, Deserialize, Default)]
pub struct LogsQuery { pub service: Option<String>, pub limit: Option<usize>, pub since_minutes: Option<u64>, pub priority: Option<String>, pub search: Option<String> }
#[derive(Debug, Clone, Serialize)]
struct LogEntry { id:String, timestamp:f64, priority:u8, level:String, unit:Option<String>, identifier:Option<String>, pid:Option<String>, boot_id:Option<String>, message:String }
#[derive(Debug, Clone, Serialize)]
struct LogService { key:&'static str, title:&'static str, units:&'static [&'static str] }
#[derive(Debug, Clone)]
struct NormalizedLogsQuery { service:String, limit:usize, since_minutes:u64, priority:String, search:Option<String> }

const LOG_SERVICES:&[LogService]=&[
 LogService{key:"photoos",title:"PhotoOS Ana Servis",units:&["photoos.service"]},
 LogService{key:"storage",title:"Depolama Servisleri",units:&["photoos-storage-core.service","photoos-storage-helper.service"]},
 LogService{key:"notifications",title:"Bildirim Servisleri",units:&["photoos-notification-aggregator.service","photoos-storage-notification-producer.service"]},
 LogService{key:"update",title:"Güncelleme Servisi",units:&["photoos-update-agent.service"]},
 LogService{key:"installer",title:"Kurulum Servisleri",units:&["photoos-firstboot-installer.service","photoos-setup-provisioner.service"]},
 LogService{key:"ssh",title:"SSH",units:&["ssh.service"]},
];

fn now()->f64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs_f64() }
fn priority_level(p:u8)->&'static str { match p {0=>"emergency",1=>"alert",2=>"critical",3=>"error",4=>"warning",5=>"notice",6=>"info",7=>"debug",_=>"unknown"} }
fn service_units(key:&str)->Option<Vec<&'static str>> {
 if key=="all" { let mut seen=HashSet::new(); let mut out=Vec::new(); for s in LOG_SERVICES { for u in s.units { if seen.insert(*u){out.push(*u);} } } return Some(out); }
 LOG_SERVICES.iter().find(|s|s.key==key).map(|s|s.units.to_vec())
}
fn normalize_query(q:&LogsQuery)->Result<NormalizedLogsQuery,String> {
 let service=q.service.as_deref().unwrap_or("photoos").trim().to_ascii_lowercase(); if service_units(&service).is_none(){return Err("Bilinmeyen log servisi.".into())}
 let priority=q.priority.as_deref().unwrap_or("all").trim().to_ascii_lowercase();
 const P:&[&str]=&["all","emergency","alert","critical","error","warning","notice","info","debug"]; if !P.contains(&priority.as_str()){return Err("Bilinmeyen log seviyesi.".into())}
 let search=q.search.as_deref().map(str::trim).filter(|x|!x.is_empty()).map(str::to_ascii_lowercase);
 Ok(NormalizedLogsQuery{service,limit:q.limit.unwrap_or(200).clamp(1,2000),since_minutes:q.since_minutes.unwrap_or(120).clamp(1,43200),priority,search})
}
fn strfield(v:&Value,k:&str)->Option<String>{match v.get(k){Some(Value::String(x))=>Some(x.clone()),Some(Value::Number(x))=>Some(x.to_string()),Some(Value::Bool(x))=>Some(x.to_string()),_=>None}}
fn parse_journal_line(line:&str)->Option<LogEntry>{
 let v:Value=serde_json::from_str(line).ok()?; let raw=strfield(&v,"__REALTIME_TIMESTAMP")?; let us=raw.parse::<u64>().ok()?; let timestamp=us as f64/1_000_000.0;
 let priority=strfield(&v,"PRIORITY").and_then(|x|x.parse().ok()).unwrap_or(255); let unit=strfield(&v,"_SYSTEMD_UNIT"); let identifier=strfield(&v,"SYSLOG_IDENTIFIER"); let pid=strfield(&v,"_PID"); let boot_id=strfield(&v,"_BOOT_ID"); let message=strfield(&v,"MESSAGE").unwrap_or_default();
 let id=strfield(&v,"__CURSOR").unwrap_or_else(||format!("{}:{}:{}",raw,pid.as_deref().unwrap_or("-"),identifier.as_deref().or(unit.as_deref()).unwrap_or("system")));
 Some(LogEntry{id,timestamp,priority,level:priority_level(priority).into(),unit,identifier,pid,boot_id,message})
}
fn matches(e:&LogEntry,q:&NormalizedLogsQuery)->bool{ if q.priority!="all"&&e.level!=q.priority{return false} let Some(s)=q.search.as_deref() else{return true}; format!("{} {} {}",e.message,e.unit.as_deref().unwrap_or_default(),e.identifier.as_deref().unwrap_or_default()).to_ascii_lowercase().contains(s) }
async fn read_logs(q:&NormalizedLogsQuery)->Result<Vec<LogEntry>,String>{
 if !Path::new(JOURNALCTL).is_file(){return Err("journalctl bulunamadı.".into())} let units=service_units(&q.service).ok_or_else(||"Bilinmeyen log servisi.".to_string())?; let fetch=q.limit.saturating_mul(5).clamp(q.limit,10000);
 let mut c=Command::new(JOURNALCTL); c.arg("--no-pager").arg("--output=json").arg("--reverse").arg("--since").arg(format!("{} minutes ago",q.since_minutes)).arg("-n").arg(fetch.to_string()); for u in units { c.arg("-u").arg(u); }
 let o=c.output().await.map_err(|e|format!("journalctl çalıştırılamadı: {e}"))?; if !o.status.success(){let s=String::from_utf8_lossy(&o.stderr).trim().to_string(); return Err(if s.is_empty(){format!("journalctl başarısız: {}",o.status)}else{s})}
 let mut out=Vec::new(); for line in String::from_utf8_lossy(&o.stdout).lines(){if let Some(e)=parse_journal_line(line){if matches(&e,q){out.push(e);if out.len()>=q.limit{break}}}} Ok(out)
}
fn err(status:StatusCode,message:String)->(StatusCode,Json<Value>){(status,Json(json!({"ok":false,"message":message})))}
pub async fn list_logs(Query(q):Query<LogsQuery>)->(StatusCode,Json<Value>){let q=match normalize_query(&q){Ok(x)=>x,Err(e)=>return err(StatusCode::BAD_REQUEST,e)};match read_logs(&q).await{Ok(logs)=>(StatusCode::OK,Json(json!({"ok":true,"logs":logs,"generated_at":now()}))),Err(e)=>err(StatusCode::SERVICE_UNAVAILABLE,e)}}
pub async fn log_services()->(StatusCode,Json<Value>){(StatusCode::OK,Json(json!({"ok":true,"services":LOG_SERVICES})))}
pub async fn log_summary(Query(q):Query<LogsQuery>)->(StatusCode,Json<Value>){
 let q=match normalize_query(&q){Ok(x)=>x,Err(e)=>return err(StatusCode::BAD_REQUEST,e)}; let logs=match read_logs(&q).await{Ok(x)=>x,Err(e)=>return err(StatusCode::SERVICE_UNAVAILABLE,e)};
 let(mut emergency,mut alert,mut critical,mut error,mut warning,mut notice,mut info,mut debug,mut unknown)=(0u64,0u64,0u64,0u64,0u64,0u64,0u64,0u64,0u64);
 for x in &logs{match x.level.as_str(){"emergency"=>emergency+=1,"alert"=>alert+=1,"critical"=>critical+=1,"error"=>error+=1,"warning"=>warning+=1,"notice"=>notice+=1,"info"=>info+=1,"debug"=>debug+=1,_=>unknown+=1}}
 (StatusCode::OK,Json(json!({"ok":true,"total":logs.len(),"emergency":emergency,"alert":alert,"critical":critical,"error":error,"warning":warning,"notice":notice,"info":info,"debug":debug,"unknown":unknown,"generated_at":now()})))
}
pub async fn log_health()->(StatusCode,Json<Value>){let ok=Path::new(JOURNALCTL).is_file();(if ok{StatusCode::OK}else{StatusCode::SERVICE_UNAVAILABLE},Json(json!({"ok":ok,"status":if ok{"healthy"}else{"unavailable"},"journalctl":JOURNALCTL})))}

// PHOTOOS PHASE2B2 LOG TESTS
#[cfg(test)]
mod tests {
 use super::*;
 #[test] fn parses_systemd_journal_json_contract(){let line=r#"{"__CURSOR":"s=cursor-1","__REALTIME_TIMESTAMP":"1720000000123456","PRIORITY":"3","_SYSTEMD_UNIT":"photoos.service","SYSLOG_IDENTIFIER":"photoos","_PID":"1234","_BOOT_ID":"boot-1","MESSAGE":"example failure"}"#;let e=parse_journal_line(line).unwrap();assert_eq!(e.id,"s=cursor-1");assert!((e.timestamp-1720000000.123456).abs()<0.000001);assert_eq!(e.priority,3);assert_eq!(e.level,"error");assert_eq!(e.unit.as_deref(),Some("photoos.service"));assert_eq!(e.identifier.as_deref(),Some("photoos"));assert_eq!(e.pid.as_deref(),Some("1234"));assert_eq!(e.boot_id.as_deref(),Some("boot-1"));assert_eq!(e.message,"example failure");assert!(parse_journal_line("{broken-json").is_none());}
 #[test] fn priority_mapping_matches_syslog_contract(){assert_eq!(priority_level(0),"emergency");assert_eq!(priority_level(1),"alert");assert_eq!(priority_level(2),"critical");assert_eq!(priority_level(3),"error");assert_eq!(priority_level(4),"warning");assert_eq!(priority_level(5),"notice");assert_eq!(priority_level(6),"info");assert_eq!(priority_level(7),"debug");assert_eq!(priority_level(99),"unknown");}
 #[test] fn query_is_bounded_and_whitelisted(){let q=normalize_query(&LogsQuery{service:Some("photoos".into()),limit:Some(99999),since_minutes:Some(0),priority:Some("warning".into()),search:Some("  Disk Error  ".into())}).unwrap();assert_eq!(q.service,"photoos");assert_eq!(q.limit,2000);assert_eq!(q.since_minutes,1);assert_eq!(q.priority,"warning");assert_eq!(q.search.as_deref(),Some("disk error"));assert!(normalize_query(&LogsQuery{service:Some("../../etc".into()),..Default::default()}).is_err());assert!(normalize_query(&LogsQuery{priority:Some("bad".into()),..Default::default()}).is_err());let d=normalize_query(&LogsQuery::default()).unwrap();assert_eq!(d.service,"photoos");assert_eq!(d.limit,200);assert_eq!(d.since_minutes,120);assert_eq!(d.priority,"all");let thirty_days=normalize_query(&LogsQuery{since_minutes:Some(43200),..Default::default()}).unwrap();assert_eq!(thirty_days.since_minutes,43200);let capped=normalize_query(&LogsQuery{since_minutes:Some(99999),..Default::default()}).unwrap();assert_eq!(capped.since_minutes,43200);}
 #[test] fn service_whitelist_rejects_arbitrary_units(){assert!(service_units("photoos").unwrap().contains(&"photoos.service"));assert!(service_units("all").unwrap().contains(&"photoos-storage-core.service"));assert!(service_units("../../../root").is_none());assert!(service_units("docker.service").is_none());}
}
