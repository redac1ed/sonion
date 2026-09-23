document.getElementById("btn").addEventListener("click", async function() {
    const output = document.getElementById("output");
    try {
        const res = await fetch("/data.json");
        const data = await res.json();
        output.textContent = JSON.stringify(data, null, 2);
    } catch (e) {
        out.textContent = "smth wrong";
    }
});