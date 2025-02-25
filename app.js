// Replace this with your Rust backend's URL
const API_URL = "http://127.0.0.1:8080";

// Fetch and display all albums
function fetchAlbums() {
  fetch(`${API_URL}/albums`)
    .then((response) => response.json())
    .then((data) => {
      console.log("Fetched albums:", data);
      const recordList = document.getElementById("recordList");
      recordList.innerHTML = "";
      document.getElementById("recordCount").textContent = `Total Albums: ${data.length}`;

      data.forEach((album) => {
        const item = document.createElement("li");
        item.classList.add("list-group-item", "d-flex", "align-items-center", "justify-content-between");
        item.innerHTML = `
          <div style='display:flex; align-items:center;'>
            <img src="${album.cover_url || 'https://via.placeholder.com/50'}"
                 alt="${album.title}" 
                 style='width:50px; height:50px; margin-right:10px;'>
            <strong>${album.title}</strong> by ${album.artist}
          </div> 
          <!-- Delete Button -->
          <button data-id="${album.id}" 
                  onclick='deleteAlbum(${album.id})' 
                  class='btn btn-danger btn-sm'>Delete</button>`;
        recordList.appendChild(item);
      });
    })
    .catch((error) => console.error("Error fetching albums:", error));
}

// Add a new album
document.getElementById("addAlbumForm").addEventListener("submit", function (event) {
  event.preventDefault();

  const title = document.getElementById("albumTitle").value;
  const artist = document.getElementById("artistName").value;
  const coverUrl = document.getElementById("albumCoverUrl").value || null;

  fetch(`${API_URL}/albums`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ title, artist, cover_url: coverUrl }),
  })
    .then((response) => response.text())
    .then(() => {
      alert("Album added successfully!");
      fetchAlbums(); // Refresh the list of albums
      bootstrap.Modal.getInstance(document.getElementById('addAlbumModal')).hide();
      document.getElementById("addAlbumForm").reset();
    })
    .catch((error) => console.error("Error adding album:", error));
});

// Delete an album
function deleteAlbum(albumId) {
  if (!confirm("Are you sure you want to delete this album?")) return;

  fetch(`${API_URL}/albums/${albumId}`, { method: "DELETE" })
    .then((response) => response.text())
    .then(() => {
      alert("Album deleted successfully!");
      fetchAlbums(); // Refresh the list of albums
    })
    .catch((error) => console.error("Error deleting album:", error));
}

// Fetch albums on page load
fetchAlbums();
