import SwiftUI

struct ContentView: View {
    @State private var url = ""

    var body: some View {
        Form {
            Section { TextField("URL", text: $url) }
            Button(action: { print(url) }) {
                Text("Download").frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
            .clipShape(.capsule)
            .listRowInsets(.init())
        }
    }
}

#Preview {
    ContentView()
}
