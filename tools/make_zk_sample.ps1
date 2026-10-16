# Builds a ZKTime 5.0 style att2000.mdb (Access 2000 / Jet 4) with fake people and punches, for testing.
$ErrorActionPreference = "Stop"
$p = Join-Path $env:GITHUB_WORKSPACE "att2000.mdb"
if (Test-Path $p) { Remove-Item $p }
$prov = "Microsoft.Jet.OLEDB.4.0"
$cat = New-Object -ComObject ADOX.Catalog
$cat.Create("Provider=$prov;Data Source=$p;Jet OLEDB:Engine Type=5") | Out-Null
$cn = New-Object -ComObject ADODB.Connection
$cn.Open("Provider=$prov;Data Source=$p")
$cn.Execute("CREATE TABLE DEPARTMENTS (DEPTID COUNTER PRIMARY KEY, DEPTNAME VARCHAR(30), SUPDEPTID INTEGER)") | Out-Null
$cn.Execute("CREATE TABLE USERINFO (USERID COUNTER PRIMARY KEY, Badgenumber VARCHAR(24), SSN VARCHAR(20), Name VARCHAR(40), Gender VARCHAR(8), TITLE VARCHAR(20), PAGER VARCHAR(20), BIRTHDAY DATETIME, HIREDDAY DATETIME, street VARCHAR(80), CITY VARCHAR(2), STATE VARCHAR(2), ZIP VARCHAR(12), OPHONE VARCHAR(20), FPHONE VARCHAR(20), VERIFICATIONMETHOD SMALLINT, DEFAULTDEPTID SMALLINT, SECURITYFLAGS SMALLINT, ATT SMALLINT, INLATE SMALLINT, OUTEARLY SMALLINT, OVERTIME SMALLINT, SEP SMALLINT, HOLIDAY SMALLINT, MINZU VARCHAR(8), PASSWORD VARCHAR(20), LUNCHDURATION SMALLINT, privilege INTEGER)") | Out-Null
$cn.Execute("CREATE TABLE CHECKINOUT (USERID INTEGER, CHECKTIME DATETIME, CHECKTYPE VARCHAR(1), VERIFYCODE INTEGER, SENSORID VARCHAR(5), Memoinfo VARCHAR(30), WorkCode VARCHAR(24), sn VARCHAR(20), UserExtFmt SMALLINT)") | Out-Null
$cn.Execute("INSERT INTO DEPARTMENTS (DEPTNAME, SUPDEPTID) VALUES ('NB Sons', 0)") | Out-Null
# Badge numbers as ZKT stores them (plain numbers); names as typed on the machine
$people = @(
  @("101","Ali Raza"), @("102","Bilal Ahmed"), @("103","Hina Tariq"), @("104","Usman Ghani"), @("105","Sana Iqbal"),
  @("106","Kamran Shah"), @("107","Ayesha Noor"), @("108","Imran Butt"), @("109","Zainab Ali"), @("110","Faisal Mehmood"),
  @("201","Visitor Guard"), @("12","H AFTAB")
)
foreach ($u in $people) {
  $cn.Execute("INSERT INTO USERINFO (Badgenumber, Name, Gender, DEFAULTDEPTID, ATT, INLATE, OUTEARLY, OVERTIME, SEP, HOLIDAY, LUNCHDURATION, privilege) VALUES ('$($u[0])', '$($u[1])', 'Male', 1, 1, 1, 1, 1, 1, 1, 1, 0)") | Out-Null
}
$rnd = New-Object System.Random 42
$today = (Get-Date).Date
$n = 0
for ($d = 20; $d -ge 0; $d--) {
  $day = $today.AddDays(-$d)
  if ($day.DayOfWeek -eq "Sunday") { continue }
  for ($uid = 1; $uid -le $people.Count; $uid++) {
    if ($rnd.Next(0, 12) -eq 0) { continue }                       # absent sometimes
    $in = $day.AddHours(8).AddMinutes(40 + $rnd.Next(0, 65)).AddSeconds($rnd.Next(0, 59))
    $cn.Execute("INSERT INTO CHECKINOUT (USERID, CHECKTIME, CHECKTYPE, VERIFYCODE, SENSORID, sn) VALUES ($uid, #$($in.ToString('MM/dd/yyyy HH:mm:ss'))#, 'I', 1, '1', 'ZKSAMPLE')") | Out-Null; $n++
    if ($rnd.Next(0, 4) -eq 0) {                                    # double press at the door
      $dup = $in.AddSeconds(25)
      $cn.Execute("INSERT INTO CHECKINOUT (USERID, CHECKTIME, CHECKTYPE, VERIFYCODE, SENSORID, sn) VALUES ($uid, #$($dup.ToString('MM/dd/yyyy HH:mm:ss'))#, 'I', 1, '1', 'ZKSAMPLE')") | Out-Null; $n++
    }
    if ($d -eq 0) { continue }                                      # today: still at work
    $out = $day.AddHours(17).AddMinutes($rnd.Next(0, 90))
    $cn.Execute("INSERT INTO CHECKINOUT (USERID, CHECKTIME, CHECKTYPE, VERIFYCODE, SENSORID, sn) VALUES ($uid, #$($out.ToString('MM/dd/yyyy HH:mm:ss'))#, 'O', 1, '1', 'ZKSAMPLE')") | Out-Null; $n++
  }
}
$cn.Close()
Write-Host "att2000.mdb created with $($people.Count) people and $n punches ($((Get-Item $p).Length) bytes)"
