// Fetch and display all albums
async function fetchAlbums() {
    const response = await fetch('/albums');
    const albums = await response.json();
  
    const albumList = document.getElementById('album-list');
    albumList.innerHTML = ''; // Clear existing list
  
    albums.forEach(album => {
        const li = document.createElement('li');
        li.textContent = `${album.title} by ${album.artist}`;
        li.addEventListener('click', () => fetchPlayHistory(album.album_id));
        albumList.appendChild(li);
    });
  }
  
  // Add a new album via POST request
  async function addAlbum(event) {
    event.preventDefault();
  
    const title = document.getElementById('title').value;
    const artist = document.getElementById('artist').value;
    const coverUrl = document.getElementById('cover-url').value || 'default-cover.jpg';
  
    const response = await fetch('/albums', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ title, artist, cover_url: coverUrl }),
    });
  
    if (response.ok) {
        alert('Album added successfully!');
        fetchAlbums();
    } else {
        alert('Failed to add album.');
    }
  }
  
  // Fetch and display play history for an album
  async function fetchPlayHistory(albumId) {
    const response = await fetch(`/play_history/${albumId}`);
    const history = await response.json();
  
    const historyList = document.getElementById('play-history-list');
    historyList.innerHTML = ''; // Clear existing list
  
    history.forEach(entry => {
        const li = document.createElement('li');
        li.textContent = `Played on ${entry.played_on}`;
        historyList.appendChild(li);
    });
  }
  
  // Attach event listeners
  document.getElementById('add-album-form').addEventListener('submit', addAlbum);
  
  // Initial data load
  fetchAlbums();
  