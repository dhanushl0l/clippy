pipeline {
    agent any
    stages {
        stage("Git Checkout Stage") {
            steps {
                git branch: 'main', url: 'https://github.com/dhanushl0l/clippy.git'
                echo 'Checkout Successful ✅'
            }
        }
        
        stage('Docker image Build and Docker compose Up') {
            steps {
                sh 'sudo docker compose build --no-cache -d'
                echo 'Docker compose Up Successfully ✅'
            }
        }
    }
}