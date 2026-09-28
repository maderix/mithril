fn run(k: i64) -> i64 {
    let n = 20000usize;
    let mut s = 0i64;
    for r in 0..k {
        let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
        let mut x = r + 1;
        for _ in 0..30000 {
            x = (x * 1103515245 + 12345) & 2147483647; let u = (x % n as i64) as usize;
            x = (x * 1103515245 + 12345) & 2147483647; let v = (x % n as i64) as usize;
            adj[u].push(v); adj[v].push(u);
        }
        let mut seen = vec![false; n];
        let (mut c, mut h) = (0i64, 0i64);
        for u in 0..n {
            if !seen[u] {
                let mut st = vec![u];
                while let Some(w) = st.pop() {
                    if seen[w] { continue; }
                    seen[w] = true;
                    h = (h * 31 + w as i64) & 4294967295;
                    // the Mithril version pushes the cons list (newest first)
                    // so the stack pops it in insertion order: same traversal
                    for &y in adj[w].iter().rev() { st.push(y); }
                }
                c += 1;
            }
        }
        s = (s + ((c * 1000003 + h) & 4294967295)) & 4294967295;
    }
    s
}
fn main() { println!("{}", run(std::env::args().nth(1).map(|a| a.parse().unwrap()).unwrap_or(600))); }
